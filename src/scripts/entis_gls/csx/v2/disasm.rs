use super::types::*;
use crate::ext::io::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::collections::HashMap;
use std::io::{Seek, Write};

fn escape_string(s: &str) -> String {
    s.replace("\\", "\\\\")
        .replace("\r", "\\r")
        .replace("\n", "\\n")
        .replace("\t", "\\t")
}

#[allow(dead_code)]
pub struct ECSExecutionImageDisassembler<'a> {
    pub stream: MemReaderRef<'a>,
    function: &'a SectionFunction,
    func_info: &'a SectionFuncInfo,
    import_native_func: &'a SectionImportNativeFunc,
    class_info: &'a SectionClassInfo,
    const_string: &'a SectionConstString,
    pub assembly: ECSExecutionImageAssembly,
    writer: Option<Box<dyn Write + 'a>>,
    addr: u32,
    code: CSInstructionCode,
    pub func_map: HashMap<u32, &'a FuncInfoEntry>,
}

impl<'a> ECSExecutionImageDisassembler<'a> {
    pub fn new(
        stream: MemReaderRef<'a>,
        function: &'a SectionFunction,
        func_info: &'a SectionFuncInfo,
        import_native_func: &'a SectionImportNativeFunc,
        class_info: &'a SectionClassInfo,
        const_string: &'a SectionConstString,
        writer: Option<Box<dyn Write + 'a>>,
    ) -> Self {
        Self {
            stream,
            function,
            func_info,
            import_native_func,
            class_info,
            const_string,
            assembly: ECSExecutionImageAssembly {
                commands: Vec::new(),
            },
            writer,
            addr: 0,
            code: CsicNew,
            func_map: func_info
                .functions
                .iter()
                .map_while(|x| {
                    if x.header.bytes == 0 {
                        None
                    } else {
                        Some((x.header.address, x))
                    }
                })
                .collect(),
        }
    }

    pub fn execute(&mut self) -> Result<()> {
        let mut ordered = self.func_info.functions.clone();
        ordered.sort_by_key(|f| f.header.address);
        let img_len = self.stream.data.len() as u32;
        let mut pre_end = 0;
        for e in ordered {
            // ignore inline functions
            if e.header.flags == 0x11 {
                eprintln!(
                    "Skipping inline function at {:#x}, {}",
                    e.header.address, e.name.0
                );
                continue;
            }
            let start = e.header.address;
            if e.header.bytes == u32::MAX {
                // eprintln!("function at {:#x} has invalid size, {}", start, e.name.0);
                continue;
            }
            let end = start + e.header.bytes;
            if end > img_len {
                eprintln!(
                    "Warning: function end {:#x} exceeds image length {:#x}",
                    end, img_len
                );
                crate::COUNTER.inc_warning();
                continue;
            }
            if pre_end != 0 && pre_end < start {
                self.assembly.push(ECSExecutionImageCommandRecord {
                    code: CodeSystemReserved,
                    addr: pre_end,
                    size: start - pre_end,
                    new_addr: pre_end,
                    internal: true,
                });
            }
            self.execute_range(start, end)?;
            pre_end = end;
        }
        if pre_end != 0 && pre_end < img_len {
            self.assembly.push(ECSExecutionImageCommandRecord {
                code: CodeSystemReserved,
                addr: pre_end,
                size: img_len - pre_end,
                new_addr: pre_end,
                internal: true,
            });
        }
        if self.func_info.functions.is_empty() {
            // older format without function info
            // try to disassemble the whole section
            // assuming there are no padding bytes
            self.execute_range(0, img_len)?;
        }
        Ok(())
    }

    fn line<S: AsRef<str> + ?Sized>(&mut self, line: &S) -> anyhow::Result<()> {
        if let Some(writer) = &mut self.writer {
            writeln!(writer, "{:08x} {}", self.addr, line.as_ref())?;
        }
        Ok(())
    }

    fn execute_range(&mut self, start: u32, end: u32) -> Result<()> {
        self.stream.pos = start as usize;
        let end = end as usize;
        // println!("Disassembling range {:#08x} - {:#08x}", start, end);
        while self.stream.pos < end {
            self.addr = self.stream.pos as u32;
            let code = self.stream.read_u8()?;
            self.code = CSInstructionCode::try_from(code).map_err(|_| {
                anyhow::anyhow!(
                    "Invalid CSInstructionCode value: {} at {:08x}",
                    code,
                    self.addr
                )
            })?;
            match self.code {
                CsicNew => self.command_new()?,
                CsicFree => self.command_free()?,
                CsicLoad => self.command_load()?,
                CsicStore => self.command_store()?,
                CsicEnter => self.command_enter()?,
                CsicLeave => self.command_leave()?,
                CsicJump => self.command_jump()?,
                CsicCJump => self.command_cjump()?,
                CsicCall => self.command_call()?,
                CsicReturn => self.command_return()?,
                CsicElement => self.command_element()?,
                CsicElementIndirect => self.command_element_indirect()?,
                CsicOperate => self.command_operate()?,
                CsicUniOperate => self.command_uni_operate()?,
                CsicCompare => self.command_compare()?,
                CsicExOperate => self.command_ex_operate()?,
                CsicExUniOperate => self.command_ex_uni_operate()?,
                CsicExCall => self.command_ex_call()?,
                CsicExReturn => self.command_ex_return()?,
                CsicCallMember => self.command_call_member()?,
                CsicCallNativeMember => self.command_call_native_member()?,
                CsicSwap => self.command_swap()?,
                CsicCreateBufferVSize => self.command_create_buffer_vsize()?,
                CsicPointerToObject => self.command_pointer_to_object()?,
                CsicReferenceForPointer => self.command_reference_for_pointer()?,
                CsicCallNativeFunction => self.command_call_native_function()?,
                CodeLoadMem => self.shell_command_load_mem()?,
                CodeLoadMemBaseImm32 => self.shell_command_load_mem_base_imm32()?,
                CodeLoadMemBaseIndex => self.shell_command_load_mem_base_index()?,
                CodeLoadMemBaseIndexImm32 => self.shell_command_load_mem_base_index_imm32()?,
                CodeStoreMem => self.shell_command_store_mem()?,
                CodeStoreMemBaseImm32 => self.shell_command_store_mem_base_imm32()?,
                CodeStoreMemBaseIndex => self.shell_command_store_mem_base_index()?,
                CodeStoreMemBaseIndexImm32 => self.shell_command_store_mem_base_index_imm32()?,
                CodeLoadLocal => self.shell_command_load_local()?,
                CodeLoadLocalIndexImm32 => self.shell_command_load_local_index_imm32()?,
                CodeStoreLocal => self.shell_command_store_local()?,
                CodeStoreLocalIndexImm32 => self.shell_command_store_local_index_imm32()?,
                CodeMoveReg => self.shell_command_move_reg()?,
                CodeCvtFloat2Int => self.shell_command_cvt_float_2_int()?,
                CodeCvtInt2Float => self.shell_command_cvt_int_2_float()?,
                CodeSrlImm8 => self.shell_command_srl_imm8()?,
                CodeSraImm8 => self.shell_command_sra_imm8()?,
                CodeSllImm8 => self.shell_command_sll_imm8()?,
                CodeMaskMove => self.shell_command_mask_move()?,
                CodeAddImm32 => self.shell_command_add_imm32()?,
                CodeMulImm32 => self.shell_command_mul_imm32()?,
                CodeAddSPImm32 => self.shell_command_add_sp_imm32()?,
                CodeLoadImm64 => self.shell_command_load_imm64()?,
                CodeNegInt => self.shell_command_neg_int()?,
                CodeNotInt => self.shell_command_not_int()?,
                CodeNegFloat => self.shell_command_neg_float()?,
                CodeAddReg => self.shell_command_add_reg()?,
                CodeSubReg => self.shell_command_sub_reg()?,
                CodeMulReg => self.shell_command_mul_reg()?,
                CodeDivReg => self.shell_command_div_reg()?,
                CodeModReg => self.shell_command_mod_reg()?,
                CodeAndReg => self.shell_command_and_reg()?,
                CodeOrReg => self.shell_command_or_reg()?,
                CodeXorReg => self.shell_command_xor_reg()?,
                CodeSrlReg => self.shell_command_srl_reg()?,
                CodeSraReg => self.shell_command_sra_reg()?,
                CodeSllReg => self.shell_command_sll_reg()?,
                CodeMoveSx32Reg => self.shell_command_move_sx32_reg()?,
                CodeMoveSx16Reg => self.shell_command_move_sx16_reg()?,
                CodeMoveSx8Reg => self.shell_command_move_sx8_reg()?,
                CodeFAddReg => self.shell_command_f_add_reg()?,
                CodeFSubReg => self.shell_command_f_sub_reg()?,
                CodeFMulReg => self.shell_command_f_mul_reg()?,
                CodeFDivReg => self.shell_command_f_div_reg()?,
                CodeMul32Reg => self.shell_command_mul32_reg()?,
                CodeIMul32Reg => self.shell_command_i_mul32_reg()?,
                CodeDiv32Reg => self.shell_command_div32_reg()?,
                CodeIDiv32Reg => self.shell_command_i_div32_reg()?,
                CodeMod32Reg => self.shell_command_mod32_reg()?,
                CodeIMod32Reg => self.shell_command_i_mod32_reg()?,
                CodeCmpNeReg => self.shell_command_cmp_ne_reg()?,
                CodeCmpEqReg => self.shell_command_cmp_eq_reg()?,
                CodeCmpLtReg => self.shell_command_cmp_lt_reg()?,
                CodeCmpLeReg => self.shell_command_cmp_le_reg()?,
                CodeCmpGtReg => self.shell_command_cmp_gt_reg()?,
                CodeCmpGeReg => self.shell_command_cmp_ge_reg()?,
                CodeCmpCReg => self.shell_command_cmp_c_reg()?,
                CodeCmpCZReg => self.shell_command_cmp_cz_reg()?,
                CodeFCmpNeReg => self.shell_command_f_cmp_ne_reg()?,
                CodeFCmpEqReg => self.shell_command_f_cmp_eq_reg()?,
                CodeFCmpLtReg => self.shell_command_f_cmp_lt_reg()?,
                CodeFCmpLeReg => self.shell_command_f_cmp_le_reg()?,
                CodeFCmpGtReg => self.shell_command_f_cmp_gt_reg()?,
                CodeFCmpGeReg => self.shell_command_f_cmp_ge_reg()?,
                CodeJumpOffset32 => self.shell_command_jump_offset32()?,
                CodeJumpReg => self.shell_command_jump_reg()?,
                CodeCNJumpOffset32 => self.shell_command_cn_jump_offset32()?,
                CodeCJumpOffset32 => self.shell_command_c_jump_offset32()?,
                CodeCallImm32 => self.shell_command_call_imm32()?,
                CodeCallReg => self.shell_command_call_reg()?,
                CodeSysCallImm32 => self.shell_command_sys_call_imm32()?,
                CodeSysCallReg => self.shell_command_sys_call_reg()?,
                CodeReturn => self.shell_command_return()?,
                CodePushReg => self.shell_command_push_reg()?,
                CodePopReg => self.shell_command_pop_reg()?,
                CodePushRegs => self.shell_command_push_regs()?,
                CodePopRegs => self.shell_command_pop_regs()?,
                CodeMemoryHint => self.shell_command_memory_hint()?,
                CodeFloatExtension => self.shell_command_float_extension()?,
                CodeSIMD64Extension2Op => self.shell_command_simd64_extension_2op()?,
                CodeSIMD64Extension3Op => self.shell_command_simd64_extension_3op()?,
                CodeSIMD128Extension2Op => self.shell_command_simd128_extension_2op()?,
                CodeSIMD128Extension3Op => self.shell_command_simd128_extension_3op()?,
                CodeEscape => self.shell_command_escape()?,
                CodeNoOperation => self.shell_command_no_operation()?,
                CodeSystemReserved => self.shell_command_system_reserved()?,
            }
            let size = self.stream.pos as u32 - self.addr;
            self.assembly.push(ECSExecutionImageCommandRecord {
                code: self.code,
                addr: self.addr,
                size,
                new_addr: self.addr,
                internal: false,
            });
        }
        Ok(())
    }

    fn get_string_literal2(&mut self) -> Result<(Option<usize>, String)> {
        let length = self.stream.read_u32()?;
        if length != 0x80000000 {
            self.stream.seek_relative(-4)?;
            let s = WideString::unpack(&mut self.stream, false, Encoding::Utf16LE, &None)?.0;
            Ok((None, s))
        } else {
            let index = self.stream.read_u32()? as usize;
            match self.const_string.strings.get(index) {
                Some(s) => Ok((Some(index), s.string.0.clone())),
                None => Err(anyhow::anyhow!(
                    "Invalid string literal index: {} (max {})",
                    index,
                    self.const_string.strings.len()
                )),
            }
        }
    }

    pub fn get_string_literal(&mut self) -> Result<String> {
        let (_, s) = self.get_string_literal2()?;
        Ok(s)
    }

    fn read_csct(&mut self) -> Result<CSCompareType> {
        let value = self.stream.read_u8()?;
        CSCompareType::try_from(value).map_err(|_| {
            anyhow::anyhow!(
                "Invalid CSCompareType value: {} at {:08x}",
                value,
                self.addr
            )
        })
    }

    pub fn read_csom(&mut self) -> Result<CSObjectMode> {
        let value = self.stream.read_u8()?;
        CSObjectMode::try_from(value).map_err(|_| {
            anyhow::anyhow!("Invalid CSObjectMode value: {} at {:08x}", value, self.addr)
        })
    }

    pub fn read_csot(&mut self) -> Result<CSOperatorType> {
        let value = self.stream.read_u8()?;
        CSOperatorType::try_from(value).map_err(|_| {
            anyhow::anyhow!(
                "Invalid CSOperatorType value: {} at {:08x}",
                value,
                self.addr
            )
        })
    }

    fn read_csuot(&mut self) -> Result<CSUnaryOperatorType> {
        let value = self.stream.read_u8()?;
        CSUnaryOperatorType::try_from(value).map_err(|_| {
            anyhow::anyhow!(
                "Invalid CSUnaryOperatorType value: {} at {:08x}",
                value,
                self.addr
            )
        })
    }

    pub fn read_csvt(&mut self) -> Result<CSVariableType> {
        let value = self.stream.read_u8()?;
        CSVariableType::try_from(value).map_err(|_| {
            anyhow::anyhow!(
                "Invalid CSVariableType value: {} at {:08x}",
                value,
                self.addr
            )
        })
    }

    fn read_csxot(&mut self) -> Result<CSExtraOperatorType> {
        let value = self.stream.read_u8()?;
        CSExtraOperatorType::try_from(value).map_err(|_| {
            anyhow::anyhow!(
                "Invalid CSExtraOperatorType value: {} at {:08x}",
                value,
                self.addr
            )
        })
    }

    fn read_csxuot(&mut self) -> Result<CSExtraUniOperatorType> {
        let value = self.stream.read_u8()?;
        CSExtraUniOperatorType::try_from(value).map_err(|_| {
            anyhow::anyhow!(
                "Invalid CSExtraUniOperatorType value: {} at {:08x}",
                value,
                self.addr
            )
        })
    }

    fn command_new(&mut self) -> Result<()> {
        let csom = self.read_csom()?;
        let csvt = self.read_csvt()?;
        let class_name = if csvt == CsvtClassObject {
            let class_index = self.stream.read_u32()?;
            match self.class_info.names.get(class_index as usize) {
                Some(c) => c.0.clone(),
                None => {
                    return Err(anyhow::anyhow!(
                        "Invalid class index: {} (max {}) at {:08x}",
                        class_index,
                        self.class_info.names.len(),
                        self.addr
                    ));
                }
            }
        } else if csvt == CsvtObject {
            self.get_string_literal()?
        } else {
            String::new()
        };
        let var_name = self.get_string_literal()?;
        let mode = match csom {
            CsomStack => "stack",
            CsomThis => "this",
            _ => {
                return Err(anyhow::anyhow!(
                    "Unexpected CSObjectMode in command_new: {:?} at {:08x}",
                    csom,
                    self.addr
                ));
            }
        };
        if class_name.is_empty() {
            self.line(&format!("New {mode} \"{var_name}\""))?;
        } else {
            self.line(&format!("New {mode} \"{class_name}\" \"{var_name}\""))?;
        }
        Ok(())
    }

    fn command_free(&mut self) -> Result<()> {
        self.line("Free")
    }

    fn command_load(&mut self) -> Result<()> {
        let csom = self.read_csom()?;
        let csvt = self.read_csvt()?;
        if csom == CsomImmediate {
            match csvt {
                CsvtObject => {
                    let class_name = self.get_string_literal()?;
                    self.line(&format!("Load New \"{class_name}\""))?;
                }
                CsvtReference => {
                    self.line("Load New \"ECSReference\"")?;
                }
                CsvtArray => {
                    self.line("Load New \"ECSArray\"")?;
                }
                CsvtHash => {
                    self.line("Load New \"ECSHash\"")?;
                }
                CsvtInteger => {
                    let value = self.stream.read_u32()?;
                    self.line(&format!("Load Integer {value}"))?;
                }
                CsvtReal => {
                    let value = self.stream.read_f64()?;
                    self.line(&format!("Load Real {value}"))?;
                }
                CsvtString => {
                    let t = self.get_string_literal2()?;
                    let escaped = escape_string(&t.1);
                    if let Some(index) = t.0 {
                        self.line(&format!("Load Const String \"{escaped}\" ({index})"))?;
                    } else {
                        self.line(&format!("Load String \"{escaped}\""))?;
                    }
                }
                CsvtInteger64 => {
                    let value = self.stream.read_u64()?;
                    self.line(&format!("Load Integer64 {value}"))?;
                }
                CsvtPointer => {
                    let point = self.stream.read_u32()?;
                    self.line(&format!("Load Pointer {point}"))?;
                }
                CsvtClassObject => {
                    let class_index = self.stream.read_u32()?;
                    let class_name = match self.class_info.names.get(class_index as usize) {
                        Some(c) => c.0.clone(),
                        None => {
                            return Err(anyhow::anyhow!(
                                "Invalid class index: {} (max {}) at {:08x}",
                                class_index,
                                self.class_info.names.len(),
                                self.addr
                            ));
                        }
                    };
                    self.line(&format!("Load New \"{class_name}\""))?;
                }
                CsvtBoolean => {
                    let value = self.stream.read_u8()?;
                    self.line(&format!("Load Boolean {value}"))?;
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unexpected variable type in Load Immediate: {:?} at {:08x}",
                        csvt,
                        self.addr
                    ));
                }
            }
        } else {
            let mode = match csom {
                CsomStack => "stack",
                CsomThis => "this",
                CsomGlobal => "global",
                CsomData => "data",
                CsomAuto => "auto",
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unexpected CSObjectMode in command_load: {:?} at {:08x}",
                        csom,
                        self.addr
                    ));
                }
            };
            match csvt {
                CsvtReference => {
                    self.line(&format!("Load {mode}"))?;
                }
                CsvtInteger => {
                    let index = self.stream.read_i32()?;
                    self.line(&format!("Load {mode} [{index}]"))?;
                }
                CsvtString => {
                    let name = self.get_string_literal()?;
                    self.line(&format!("Load {mode} [\"{name}\"]"))?;
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unexpected variable type in Load: {:?} at {:08x}",
                        csvt,
                        self.addr
                    ));
                }
            }
        }
        Ok(())
    }

    fn command_store(&mut self) -> Result<()> {
        let csot = self.read_csot()?;
        match csot {
            CsotNop => {
                self.line("Store")?;
            }
            CsotAdd => {
                self.line("Store.Add")?;
            }
            CsotSub => {
                self.line("Store.Sub")?;
            }
            CsotMul => {
                self.line("Store.Mul")?;
            }
            CsotDiv => {
                self.line("Store.Div")?;
            }
            CsotMod => {
                self.line("Store.Mod")?;
            }
            CsotAnd => {
                self.line("Store.And")?;
            }
            CsotOr => {
                self.line("Store.Or")?;
            }
            CsotXor => {
                self.line("Store.Xor")?;
            }
            CsotLogicalAnd => {
                self.line("Store.LAnd")?;
            }
            CsotLogicalOr => {
                self.line("Store.LOr")?;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unexpected CSOperatorType in command_store: {:?} at {:08x}",
                    csot,
                    self.addr
                ));
            }
        }
        Ok(())
    }

    fn command_enter(&mut self) -> Result<()> {
        let name = self.get_string_literal()?;
        let num_args = self.stream.read_i32()?;
        if num_args != -1 {
            let mut sb = String::new();
            sb.push('(');
            for i in 0..num_args {
                let csvt = self.read_csvt()?;
                let class_name = if csvt == CsvtClassObject {
                    let class_index = self.stream.read_u32()?;
                    match self.class_info.names.get(class_index as usize) {
                        Some(c) => c.0.clone(),
                        None => {
                            return Err(anyhow::anyhow!(
                                "Invalid class index: {} (max {}) at {:08x}",
                                class_index,
                                self.class_info.names.len(),
                                self.addr
                            ));
                        }
                    }
                } else if csvt == CsvtObject {
                    self.get_string_literal()?
                } else {
                    String::new()
                };
                let var_name = self.get_string_literal()?;
                if class_name.is_empty() {
                    sb.push_str(&var_name);
                } else {
                    sb.push_str(&format!("{{{class_name}:{var_name}}}"));
                }
                if i < num_args - 1 {
                    sb.push_str(", ");
                }
            }
            sb.push(')');
            self.line(&format!("Enter \"{}\" {}", name, sb))?;
        } else {
            let flag = self.stream.read_u8()?;
            if flag != 0 {
                return Err(anyhow::anyhow!(
                    "Invalid flag for variable argument 'enter' instruction: {} at {:08x}",
                    flag,
                    self.addr
                ));
            }
            let catch_addr = self.stream.read_i32()? as i64 + self.stream.pos as i64;
            self.line(&format!("Enter \"{}\" Try-Catch {:08x}", name, catch_addr))?;
        }
        Ok(())
    }

    fn command_leave(&mut self) -> Result<()> {
        self.line("Leave")
    }

    fn command_jump(&mut self) -> Result<()> {
        let addr = self.stream.read_i32()? as i64 + self.stream.pos as i64;
        self.line(&format!("Jump {addr:08x}"))
    }

    fn command_cjump(&mut self) -> Result<()> {
        let cond = self.stream.read_u8()?;
        let addr = self.stream.read_i32()? as i64 + self.stream.pos as i64;
        self.line(&format!("CJump {cond} {addr:08x}"))
    }

    fn command_call(&mut self) -> Result<()> {
        let csom = self.read_csom()?;
        let num_args = self.stream.read_i32()?;
        let func_name = self.get_string_literal()?;
        let mode = match csom {
            CsomImmediate if func_name == "@CATCH" => "",
            CsomThis => "This",
            CsomGlobal => "Global",
            CsomAuto => "Auto",
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid CSObjectMode for 'call' instruction: {:?} {} at {:08x}",
                    csom,
                    func_name,
                    self.addr
                ));
            }
        };
        self.line(&format!("Call {mode} \"{func_name}\" <{num_args}>"))
    }

    fn command_return(&mut self) -> Result<()> {
        let free_stack = self.stream.read_u8()?;
        if free_stack == 1 {
            self.line("Return Void")
        } else {
            self.line("Return")
        }
    }

    fn command_element(&mut self) -> Result<()> {
        let csvt = self.read_csvt()?;
        match csvt {
            CsvtInteger => {
                let index = self.stream.read_i32()?;
                self.line(&format!("Element [{index}]"))
            }
            CsvtString => {
                let name = self.get_string_literal()?;
                self.line(&format!("Element [\"{name}\"]"))
            }
            _ => Err(anyhow::anyhow!(
                "Unexpected variable type in Element: {:?} at {:08x}",
                csvt,
                self.addr
            )),
        }
    }

    fn command_element_indirect(&mut self) -> Result<()> {
        self.line("ElementIndirect")
    }

    fn command_operate(&mut self) -> Result<()> {
        let csot = self.read_csot()?;
        match csot {
            CsotNop => self.line("Operate.Nop"),
            CsotAdd => self.line("Operate.Add"),
            CsotSub => self.line("Operate.Sub"),
            CsotMul => self.line("Operate.Mul"),
            CsotDiv => self.line("Operate.Div"),
            CsotMod => self.line("Operate.Mod"),
            CsotAnd => self.line("Operate.And"),
            CsotOr => self.line("Operate.Or"),
            CsotXor => self.line("Operate.Xor"),
            CsotLogicalAnd => self.line("Operate.LAnd"),
            CsotLogicalOr => self.line("Operate.LOr"),
            CsotShiftRight => self.line("Operate.ShiftRight"),
            CsotShiftLeft => self.line("Operate.ShiftLeft"),
        }
    }

    fn command_uni_operate(&mut self) -> Result<()> {
        let csuot = self.read_csuot()?;
        match csuot {
            CsuotPlus => self.line("UnaryOperate.Plus"),
            CsuotNegate => self.line("UnaryOperate.Negate"),
            CsuotBitnot => self.line("UnaryOperate.Bitnot"),
            CsuotLogicalNot => self.line("UnaryOperate.LogicalNot"),
        }
    }

    fn command_compare(&mut self) -> Result<()> {
        let csct = self.read_csct()?;
        match csct {
            CsctEqual => self.line("Compare.Equal"),
            CsctNotEqual => self.line("Compare.NotEqual"),
            CsctLessThan => self.line("Compare.LessThan"),
            CsctLessEqual => self.line("Compare.LessEqual"),
            CsctGreaterThan => self.line("Compare.GreaterThan"),
            CsctGreaterEqual => self.line("Compare.GreaterEqual"),
            CsctNotEqualPointer => self.line("Compare.NotEqualPointer"),
            CsctEqualPointer => self.line("Compare.EqualPointer"),
        }
    }

    fn command_ex_operate(&mut self) -> Result<()> {
        let csxot = self.read_csxot()?;
        match csxot {
            CsxotArrayDim => {
                let dim = self.stream.read_i32()?;
                let mut dims = Vec::with_capacity(dim as usize);
                for _ in 0..dim {
                    let size = self.stream.read_i32()?;
                    dims.push(format!("{size}"));
                }
                let text = dims.join(", ");
                self.line(&format!("ExOperate.ArrayDim {{ {text} }}"))
            }
            CsxotHashContainer => self.line("ExOperate.HashContainer"),
            CsxotMoveReference => self.line("ExOperate.MoveReference"),
        }
    }

    fn command_ex_uni_operate(&mut self) -> Result<()> {
        let csxuot = self.read_csxuot()?;
        match csxuot {
            CsxuotDeselect => self.line("ExUnaryOperate.Deselect"),
            CsxuotBoolean => self.line("ExUnaryOperate.Boolean"),
            CsxuotSizeOf => self.line("ExUnaryOperate.SizeOf"),
            CsxuotTypeOf => self.line("ExUnaryOperate.TypeOf"),
            CsxuotStaticCast => {
                let var_offset = self.stream.read_i32()?;
                let var_bounds = self.stream.read_i32()?;
                let func_offset = self.stream.read_i32()?;
                self.line(&format!(
                    "ExUnaryOperate.StaticCast {var_offset}, {var_bounds}, {func_offset}"
                ))
            }
            CsxuotDynamicCast => {
                let cast_type = self.get_string_literal()?;
                self.line(&format!("ExUnaryOperate.DynamicCast \"{cast_type}\""))
            }
            CsxuotDuplicate => self.line("ExUnaryOperate.Duplicate"),
            CsxuotDelete => self.line("ExUnaryOperate.Delete"),
            CsxuotDeleteArray => self.line("ExUnaryOperate.DeleteArray"),
            CsxuotLoadAddress => self.line("ExUnaryOperate.LoadAddress"),
            CsxuotRefAddress => self.line("ExUnaryOperate.RefAddress"),
        }
    }

    fn command_ex_call(&mut self) -> Result<()> {
        let arg_count = self.stream.read_i32()?;
        let csom = self.read_csom()?;
        let csvt = self.read_csvt()?;
        match csom {
            CsomImmediate => match csvt {
                CsvtString => {
                    let func_name = self.get_string_literal()?;
                    self.line(&format!("ExCall \"{func_name}\" <{arg_count}>"))
                }
                CsvtInteger => {
                    let func_address = self.stream.read_u32()?;
                    if let Some(func) = self.func_map.get(&func_address) {
                        self.line(&format!("ExCall \"{}\" <{arg_count}>", func.name.0))
                    } else {
                        self.line(&format!("ExCall {func_address:08x} <{arg_count}>"))
                    }
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unexpected CSVariableType in ExCall: {:?} at {:08x}",
                        csvt,
                        self.addr
                    ));
                }
            },
            _ => {
                return Err(anyhow::anyhow!(
                    "Unexpected CSObjectMode in ExCall: {:?} at {:08x}",
                    csom,
                    self.addr
                ));
            }
        }
    }

    fn command_ex_return(&mut self) -> Result<()> {
        let free_stack = self.stream.read_u8()?;
        if free_stack == 1 {
            self.line("ExReturn Void")
        } else {
            self.line("ExReturn")
        }
    }

    fn command_call_member(&mut self) -> Result<()> {
        let arg_count = self.stream.read_i32()?;
        let class_index = self.stream.read_u32()?;
        let class = self
            .class_info
            .infos
            .get(class_index as usize)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid class info index: {} (max {}) at {:08x}",
                    class_index,
                    self.class_info.infos.len(),
                    self.addr
                )
            })?;
        let func_index = self.stream.read_u32()?;
        let func = class.method_info.get(func_index as usize).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid method info index: {} (max {}) at {:08x}",
                func_index,
                class.method_info.len(),
                self.addr
            )
        })?;
        self.line(&format!(
            "CallMember \"{}\" <{arg_count}>",
            func.prototype_info.global_name.0
        ))
    }

    fn command_call_native_member(&mut self) -> Result<()> {
        let arg_count = self.stream.read_i32()?;
        let class_index = self.stream.read_u32()?;
        let class = self
            .class_info
            .infos
            .get(class_index as usize)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid class info index: {} (max {}) at {:08x}",
                    class_index,
                    self.class_info.infos.len(),
                    self.addr
                )
            })?;
        let func_index = self.stream.read_u32()?;
        let func = class.method_info.get(func_index as usize).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid method info index: {} (max {}) at {:08x}",
                func_index,
                class.method_info.len(),
                self.addr
            )
        })?;
        self.line(&format!(
            "CallNativeMember \"{}\" <{arg_count}>",
            func.prototype_info.global_name.0
        ))
    }

    fn command_swap(&mut self) -> Result<()> {
        let _byt_sub_code = self.stream.read_u8()?;
        let index1 = self.stream.read_i32()?;
        let index2 = self.stream.read_i32()?;
        self.line(&format!("Swap #{index1} #{index2}"))
    }

    fn command_create_buffer_vsize(&mut self) -> Result<()> {
        self.line("CreateBufferVSize")
    }

    fn command_pointer_to_object(&mut self) -> Result<()> {
        let var_type = self.stream.read_i32()?;
        self.line(&format!("PointerToObject {var_type}"))
    }

    fn command_reference_for_pointer(&mut self) -> Result<()> {
        let csvt = self.read_csvt()?;
        self.line(&format!("ReferenceForPointer {:?}", csvt))
    }

    fn command_call_native_function(&mut self) -> Result<()> {
        let arg_count = self.stream.read_i32()?;
        let func_index = self.stream.read_u32()?;
        let native_func = self
            .import_native_func
            .native_func
            .names
            .get(func_index as usize)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid native function index: {} (max {}) at {:08x}",
                    func_index,
                    self.import_native_func.native_func.names.len(),
                    self.addr
                )
            })?;
        self.line(&format!(
            "CallNativeFunction \"{}\" <{arg_count}>",
            native_func.0
        ))
    }

    fn shell_command_load_mem(&mut self) -> Result<()> {
        let base = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        self.line(&format!("LoadMem {base}, %{reg}"))
    }

    fn shell_command_load_mem_base_imm32(&mut self) -> Result<()> {
        let base = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        let mem = self.stream.read_i32()?;
        self.line(&format!("LoadMemBaseImm32 %{base}, %{reg}, {mem}"))
    }

    fn shell_command_load_mem_base_index(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let index = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        self.line(&format!("LoadMemBaseIndex {data_type}, %{reg}, {index}"))
    }

    fn shell_command_load_mem_base_index_imm32(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let index = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        let imm32 = self.stream.read_i32()?;
        self.line(&format!(
            "LoadMemBaseIndexImm32 {data_type}, %{reg}, {index}, {imm32}"
        ))
    }

    fn shell_command_store_mem(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        self.line(&format!("StoreMem {data_type}, %{reg}"))
    }

    fn shell_command_store_mem_base_imm32(&mut self) -> Result<()> {
        let base = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        let mem = self.stream.read_i32()?;
        self.line(&format!("StoreMemBaseImm32 %{base}, %{reg}, {mem}"))
    }

    fn shell_command_store_mem_base_index(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let index = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        self.line(&format!("StoreMemBaseIndex {data_type}, %{reg}, {index}"))
    }

    fn shell_command_store_mem_base_index_imm32(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let index = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        let imm32 = self.stream.read_i32()?;
        self.line(&format!(
            "StoreMemBaseIndexImm32 {data_type}, %{reg}, {index}, {imm32}"
        ))
    }

    fn shell_command_load_local(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        let mem = self.stream.read_i32()?;
        self.line(&format!("LoadLocal {data_type}, %{reg}, {mem}"))
    }

    fn shell_command_load_local_index_imm32(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let index = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        let imm32 = self.stream.read_i32()?;
        self.line(&format!(
            "LoadLocalIndexImm32 {data_type}, %{reg}, {index}, {imm32}"
        ))
    }

    fn shell_command_store_local(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        let mem = self.stream.read_i32()?;
        self.line(&format!("StoreLocal {data_type}, %{reg}, {mem}"))
    }

    fn shell_command_store_local_index_imm32(&mut self) -> Result<()> {
        let data_type = self.stream.read_u8()?;
        let index = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        let imm32 = self.stream.read_i32()?;
        self.line(&format!(
            "StoreLocalIndexImm32 {data_type}, %{reg}, {index}, {imm32}"
        ))
    }

    fn shell_command_move_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("MoveReg %{dst}, %{src}"))
    }

    fn shell_command_cvt_float_2_int(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CvtFloat2Int %{dst}, %{src}"))
    }

    fn shell_command_cvt_int_2_float(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CvtInt2Float %{dst}, %{src}"))
    }

    fn shell_command_srl_imm8(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        let imm = self.stream.read_u8()?;
        self.line(&format!("SrlImm8 %{dst}, %{src}, {imm}"))
    }

    fn shell_command_sra_imm8(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        let imm = self.stream.read_u8()?;
        self.line(&format!("SraImm8 %{dst}, %{src}, {imm}"))
    }

    fn shell_command_sll_imm8(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        let imm = self.stream.read_u8()?;
        self.line(&format!("SllImm8 %{dst}, %{src}, {imm}"))
    }

    fn shell_command_mask_move(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src1 = self.stream.read_u8()?;
        let src2 = self.stream.read_u8()?;
        self.line(&format!("MaskMove %{dst}, %{src1}, %{src2}"))
    }

    fn shell_command_add_imm32(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        let imm = self.stream.read_i32()?;
        self.line(&format!("AddImm32 %{dst}, %{src}, {imm}"))
    }

    fn shell_command_mul_imm32(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        let imm = self.stream.read_i32()?;
        self.line(&format!("MulImm32 %{dst}, %{src}, {imm}"))
    }

    fn shell_command_add_sp_imm32(&mut self) -> Result<()> {
        let val = self.stream.read_i32()?;
        self.line(&format!("AddSPImm32 {val}"))
    }

    fn shell_command_load_imm64(&mut self) -> Result<()> {
        let reg = self.stream.read_u8()?;
        let imm = self.stream.read_i64()?;
        self.line(&format!("LoadImm64 %{reg}, {imm}"))
    }

    fn shell_command_neg_int(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        self.line(&format!("NegInt %{dst}"))
    }

    fn shell_command_not_int(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        self.line(&format!("NotInt %{dst}"))
    }

    fn shell_command_neg_float(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        self.line(&format!("NegFloat %{dst}"))
    }

    fn shell_command_add_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("AddReg %{dst}, %{src}"))
    }

    fn shell_command_sub_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("SubReg %{dst}, %{src}"))
    }

    fn shell_command_mul_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("MulReg %{dst}, %{src}"))
    }

    fn shell_command_div_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("DivReg %{dst}, %{src}"))
    }

    fn shell_command_mod_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("ModReg %{dst}, %{src}"))
    }

    fn shell_command_and_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("AndReg %{dst}, %{src}"))
    }

    fn shell_command_or_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("OrReg %{dst}, %{src}"))
    }

    fn shell_command_xor_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("XorReg %{dst}, %{src}"))
    }

    fn shell_command_srl_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("SrlReg %{dst}, %{src}"))
    }

    fn shell_command_sra_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("SraReg %{dst}, %{src}"))
    }

    fn shell_command_sll_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("SllReg %{dst}, %{src}"))
    }

    fn shell_command_move_sx32_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("MoveSx32Reg %{dst}, %{src}"))
    }

    fn shell_command_move_sx16_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("MoveSx16Reg %{dst}, %{src}"))
    }

    fn shell_command_move_sx8_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("MoveSx8Reg %{dst}, %{src}"))
    }

    fn shell_command_f_add_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FAddReg %{dst}, %{src}"))
    }

    fn shell_command_f_sub_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FSubReg %{dst}, %{src}"))
    }

    fn shell_command_f_mul_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FMulReg %{dst}, %{src}"))
    }

    fn shell_command_f_div_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FDivReg %{dst}, %{src}"))
    }

    fn shell_command_mul32_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("Mul32Reg %{dst}, %{src}"))
    }

    fn shell_command_i_mul32_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("IMul32Reg %{dst}, %{src}"))
    }

    fn shell_command_div32_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("Div32Reg %{dst}, %{src}"))
    }

    fn shell_command_i_div32_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("IDiv32Reg %{dst}, %{src}"))
    }

    fn shell_command_mod32_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("Mod32Reg %{dst}, %{src}"))
    }

    fn shell_command_i_mod32_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("IMod32Reg %{dst}, %{src}"))
    }

    fn shell_command_cmp_ne_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CmpNeReg %{dst}, %{src}"))
    }

    fn shell_command_cmp_eq_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CmpEqReg %{dst}, %{src}"))
    }

    fn shell_command_cmp_lt_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CmpLtReg %{dst}, %{src}"))
    }

    fn shell_command_cmp_le_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CmpLeReg %{dst}, %{src}"))
    }

    fn shell_command_cmp_gt_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CmpGtReg %{dst}, %{src}"))
    }

    fn shell_command_cmp_ge_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CmpGeReg %{dst}, %{src}"))
    }

    fn shell_command_cmp_c_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CmpCReg %{dst}, %{src}"))
    }

    fn shell_command_cmp_cz_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("CmpCZReg %{dst}, %{src}"))
    }

    fn shell_command_f_cmp_ne_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FCmpNeReg %{dst}, %{src}"))
    }

    fn shell_command_f_cmp_eq_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FCmpEqReg %{dst}, %{src}"))
    }

    fn shell_command_f_cmp_lt_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FCmpLtReg %{dst}, %{src}"))
    }

    fn shell_command_f_cmp_le_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FCmpLeReg %{dst}, %{src}"))
    }

    fn shell_command_f_cmp_gt_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FCmpGtReg %{dst}, %{src}"))
    }

    fn shell_command_f_cmp_ge_reg(&mut self) -> Result<()> {
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("FCmpGeReg %{dst}, %{src}"))
    }

    fn shell_command_jump_offset32(&mut self) -> Result<()> {
        let offset = self.stream.read_i32()? as i64;
        let dest = self.addr as i64 + offset + 5;
        self.line(&format!("JumpOffset32 {dest:08x}"))
    }

    fn shell_command_jump_reg(&mut self) -> Result<()> {
        let reg = self.stream.read_u8()?;
        self.line(&format!("JumpReg %{reg}"))
    }

    fn shell_command_cn_jump_offset32(&mut self) -> Result<()> {
        let reg = self.stream.read_u8()?;
        let offset = self.stream.read_i32()? as i64;
        let dest = self.addr as i64 + offset + 6;
        self.line(&format!("CNJumpOffset32 %{reg}, {dest:08x}"))
    }

    fn shell_command_c_jump_offset32(&mut self) -> Result<()> {
        let reg = self.stream.read_u8()?;
        let offset = self.stream.read_i32()? as i64;
        let dest = self.addr as i64 + offset + 6;
        self.line(&format!("CJumpOffset32 %{reg}, {dest:08x}"))
    }

    fn shell_command_call_imm32(&mut self) -> Result<()> {
        let dst = self.stream.read_u32()?;
        if let Some(func) = self.func_map.get(&dst) {
            self.line(&format!("CallImm32 \"{}\"", func.name.0))
        } else {
            self.line(&format!("CallImm32 {dst:08x}"))
        }
    }

    fn shell_command_call_reg(&mut self) -> Result<()> {
        let reg = self.stream.read_u8()?;
        self.line(&format!("CallReg %{reg}"))
    }

    fn shell_command_sys_call_imm32(&mut self) -> Result<()> {
        let num = self.stream.read_i32()?;
        self.line(&format!("SysCallImm32 {num:02x}"))
    }

    fn shell_command_sys_call_reg(&mut self) -> Result<()> {
        let reg = self.stream.read_u8()?;
        self.line(&format!("SysCallReg %{reg}"))
    }

    fn shell_command_return(&mut self) -> Result<()> {
        self.line("Shell Return")
    }

    fn shell_command_push_reg(&mut self) -> Result<()> {
        let reg = self.stream.read_u8()?;
        self.line(&format!("PushReg %{reg}"))
    }

    fn shell_command_pop_reg(&mut self) -> Result<()> {
        let reg = self.stream.read_u8()?;
        self.line(&format!("PopReg %{reg}"))
    }

    fn shell_command_push_regs(&mut self) -> Result<()> {
        let reg_first = self.stream.read_u8()?;
        let count = self.stream.read_u8()?;
        self.line(&format!("PushRegs %{reg_first}, {count}"))
    }

    fn shell_command_pop_regs(&mut self) -> Result<()> {
        let reg_first = self.stream.read_u8()?;
        let count = self.stream.read_u8()?;
        self.line(&format!("PopRegs %{reg_first}, {count}"))
    }

    fn shell_command_memory_hint(&mut self) -> Result<()> {
        let hint = self.stream.read_u8()?;
        let reg = self.stream.read_u8()?;
        self.line(&format!("MemoryHint {hint}, %{reg}"))
    }

    fn shell_command_float_extension(&mut self) -> Result<()> {
        let ext = self.stream.read_u8()?;
        let reg1 = self.stream.read_u8()?;
        let reg2 = self.stream.read_u8()?;
        self.line(&format!("FloatExtension {ext}, %{reg1}, %{reg2}"))
    }

    fn shell_command_simd64_extension_2op(&mut self) -> Result<()> {
        let ext = self.stream.read_u8()?;
        let reg1 = self.stream.read_u8()?;
        let reg2 = self.stream.read_u8()?;
        self.line(&format!("SIMD64Extension2Op {ext}, %{reg1}, %{reg2}"))
    }

    fn shell_command_simd64_extension_3op(&mut self) -> Result<()> {
        let ext = self.stream.read_u8()?;
        let reg1 = self.stream.read_u8()?;
        let reg2 = self.stream.read_u8()?;
        let imm = self.stream.read_u8()?;
        self.line(&format!(
            "SIMD64Extension3Op {ext}, %{reg1}, %{reg2}, {imm}"
        ))
    }

    fn shell_command_simd128_extension_2op(&mut self) -> Result<()> {
        let ext = self.stream.read_u8()?;
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        self.line(&format!("SIMD128Extension2Op {ext}, %{dst}, %{src}"))
    }

    fn shell_command_simd128_extension_3op(&mut self) -> Result<()> {
        let ext = self.stream.read_u8()?;
        let dst = self.stream.read_u8()?;
        let src = self.stream.read_u8()?;
        let imm = self.stream.read_u8()?;
        if ext == 0 {
            self.line(&format!(
                "SIMD128Extension3Op {ext}, %{dst}, %{src}, %{imm}"
            ))
        } else {
            self.line(&format!("SIMD128Extension3Op {ext}, %{dst}, %{src}, {imm}"))
        }
    }

    fn shell_command_escape(&mut self) -> Result<()> {
        self.line("Escape")
    }

    fn shell_command_no_operation(&mut self) -> Result<()> {
        self.line("NoOperation")
    }

    fn shell_command_system_reserved(&mut self) -> Result<()> {
        self.line("SystemReserved")
    }
}
