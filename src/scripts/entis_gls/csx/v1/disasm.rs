use super::types::*;
use crate::ext::io::*;
use crate::types::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use std::io::{Seek, Write};

use CSCompareType::*;
use CSInstructionCode::*;
use CSObjectMode::*;
use CSOperatorType::*;
use CSUnaryOperatorType::*;
use CSVariableType::*;

fn escape_string(s: &str) -> String {
    s.replace("\r", "\\r")
        .replace("\n", "\\n")
        .replace("\t", "\\t")
}

pub struct ECSExecutionImageDisassembler<'a> {
    pub stream: MemReaderRef<'a>,
    conststr: Option<&'a TaggedRefAddressList>,
    pub assembly: ECSExecutionImageAssembly,
    writer: Option<Box<dyn Write + 'a>>,
    addr: u32,
    code: CSInstructionCode,
}

impl<'a> ECSExecutionImageDisassembler<'a> {
    pub fn new(
        stream: MemReaderRef<'a>,
        conststr: Option<&'a TaggedRefAddressList>,
        writer: Option<Box<dyn Write + 'a>>,
    ) -> Self {
        Self {
            stream,
            conststr,
            assembly: ECSExecutionImageAssembly {
                command_list: Vec::new(),
            },
            writer,
            addr: 0,
            code: CsicNew,
        }
    }

    pub fn execute(&mut self) -> Result<()> {
        let len = self.stream.data.len();
        while self.stream.pos < len {
            self.addr = self.stream.pos as u32;
            let code_value = self.stream.read_u8()?;
            self.code = CSInstructionCode::try_from(code_value).map_err(|_| {
                anyhow::anyhow!(
                    "Invalid CSInstructionCode value: {} at {:08x}",
                    code_value,
                    self.addr
                )
            })?;
            match self.code {
                CsicNew => {
                    self.command_new()?;
                }
                CsicFree => {
                    self.command_free()?;
                }
                CsicLoad => {
                    self.command_load()?;
                }
                CsicStore => {
                    self.command_store()?;
                }
                CsicEnter => {
                    self.command_enter()?;
                }
                CsicLeave => {
                    self.command_leave()?;
                }
                CsicJump => {
                    self.command_jump()?;
                }
                CsicCJump => {
                    self.command_cjump()?;
                }
                CsicCall => {
                    self.command_call()?;
                }
                CsicReturn => {
                    self.command_return()?;
                }
                CsicElement => {
                    self.command_element()?;
                }
                CsicElementIndirect => {
                    self.command_element_indirect()?;
                }
                CsicOperate => {
                    self.command_operate()?;
                }
                CsicUniOperate => {
                    self.command_uni_operate()?;
                }
                CsicCompare => {
                    self.command_compare()?;
                }
            }
            let size = self.stream.pos as u32 - self.addr;
            self.assembly
                .command_list
                .push(ECSExecutionImageCommandRecord {
                    code: self.code,
                    addr: self.addr,
                    size,
                    new_addr: self.addr,
                });
        }
        Ok(())
    }

    fn line<S: AsRef<str> + ?Sized>(&mut self, line: &S) -> anyhow::Result<()> {
        if let Some(writer) = &mut self.writer {
            writeln!(writer, "{:08x} {}", self.addr, line.as_ref())?;
        }
        Ok(())
    }

    fn get_string_literal2(&mut self) -> Result<(Option<usize>, String)> {
        let length = self.stream.read_u32()?;
        if length != 0x80000000 {
            self.stream.seek_relative(-4)?;
            let s = WideString::unpack(&mut self.stream, false, Encoding::Utf16LE, &None)?.0;
            Ok((None, s))
        } else if let Some(conststr) = &self.conststr {
            let index = self.stream.read_u32()? as usize;
            match conststr.get(index) {
                Some(s) => Ok((Some(index), s.tag.0.clone())),
                None => Err(anyhow::anyhow!(
                    "Invalid string literal index: {} (max {})",
                    index,
                    conststr.len()
                )),
            }
        } else {
            Err(anyhow::anyhow!(
                "No constant string table for string literal index"
            ))
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

    fn read_csot(&mut self) -> Result<CSOperatorType> {
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

    fn command_new(&mut self) -> Result<()> {
        let csom = self.read_csom()?;
        let typ = self.read_csvt()?;
        let class_name = if typ == CsvtObject {
            Some(self.get_string_literal()?)
        } else {
            None
        };
        let name = self.get_string_literal()?;
        let pobj = match csom {
            CsomStack => "stack",
            CsomThis => "this",
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid CSObjectMode for 'new' instruction: {:?} at {:08x}",
                    csom,
                    self.addr
                ));
            }
        };
        match &class_name {
            Some(class_name) => {
                self.line(&format!("New {pobj} \"{class_name}\" \"{name}\""))?;
            }
            None => {
                self.line(&format!("New {pobj} \"{name}\""))?;
            }
        }
        Ok(())
    }

    fn command_free(&mut self) -> Result<()> {
        self.line("Free")?;
        Ok(())
    }

    fn command_load(&mut self) -> Result<()> {
        let csom = self.read_csom()?;
        let csvt = self.read_csvt()?;
        if csom == CsomImmediate {
            match csvt {
                CsvtObject => {
                    let class_name = self.get_string_literal()?;
                    self.line(&format!("Load * {class_name}"))?;
                }
                CsvtReference => {
                    self.line("Load * ECSReference")?;
                }
                CsvtArray => {
                    self.line("Load * ECSArray")?;
                }
                CsvtHash => {
                    self.line("Load * ECSHash")?;
                }
                CsvtInteger => {
                    let val = self.stream.read_u32()?;
                    self.line(&format!("Load Integer {}", val))?;
                }
                CsvtReal => {
                    let val = self.stream.read_f64()?;
                    self.line(&format!("Load Real {}", val))?;
                }
                CsvtString => {
                    let t = self.get_string_literal2()?;
                    let escaped = escape_string(&t.1);
                    if let Some(index) = t.0 {
                        self.line(&format!("Load Const String {index} \"{escaped}\""))?;
                    } else {
                        self.line(&format!("Load String \"{escaped}\""))?;
                    }
                }
                CsvtInteger64 => {
                    let val = self.stream.read_u64()?;
                    self.line(&format!("Load Integer64 {}", val))?;
                }
                CsvtPointer => {
                    let value = self.stream.read_u32()?;
                    self.line(&format!("Load Pointer {}", value))?;
                }
            }
        } else {
            let pobj = match csom {
                CsomStack => "stack",
                CsomThis => "this",
                CsomGlobal => "global",
                CsomData => "data",
                CsomAuto => "auto",
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid CSObjectMode for 'load' instruction: {:?} at {:08x}",
                        csom,
                        self.addr
                    ));
                }
            };
            match csvt {
                CsvtReference => {
                    self.line(&format!("Load {pobj}"))?;
                }
                CsvtInteger => {
                    let index = self.stream.read_i32()?;
                    self.line(&format!("Load {pobj} [{index}]"))?;
                }
                CsvtString => {
                    let name = self.get_string_literal()?;
                    self.line(&format!("Load {pobj} [\"{name}\"]"))?;
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid CSVariableType for 'load' instruction: {:?} at {:08x}",
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
                self.line("Store Add")?;
            }
            CsotSub => {
                self.line("Store Sub")?;
            }
            CsotMul => {
                self.line("Store Mul")?;
            }
            CsotDiv => {
                self.line("Store Div")?;
            }
            CsotMod => {
                self.line("Store Mod")?;
            }
            CsotAnd => {
                self.line("Store And")?;
            }
            CsotOr => {
                self.line("Store Or")?;
            }
            CsotXor => {
                self.line("Store Xor")?;
            }
            CsotLogicalAnd => {
                self.line("Store LAnd")?;
            }
            CsotLogicalOr => {
                self.line("Store LOr")?;
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
                let class_name = if csvt == CsvtObject {
                    Some(self.get_string_literal()?)
                } else {
                    None
                };
                let var_name = self.get_string_literal()?;
                if let Some(cname) = class_name {
                    sb.push_str(&format!("{{{}:{}}}", cname, var_name));
                } else {
                    sb.push_str(&var_name);
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
        self.line("Leave")?;
        Ok(())
    }

    fn command_jump(&mut self) -> Result<()> {
        let target_addr = self.stream.read_i32()? as i64 + self.stream.pos as i64;
        self.line(&format!("Jump {:08x}", target_addr))?;
        Ok(())
    }

    fn command_cjump(&mut self) -> Result<()> {
        let cond = self.stream.read_u8()?;
        let target_addr = self.stream.read_i32()? as i64 + self.stream.pos as i64;
        self.line(&format!("CJump {} {:08x}", cond, target_addr))?;
        Ok(())
    }

    fn command_call(&mut self) -> Result<()> {
        let csom = self.read_csom()?;
        let num_args = self.stream.read_i32()?;
        let func_name = self.get_string_literal()?;
        let pobj = match csom {
            CsomImmediate if func_name == "@CATCH" => "",
            CsomThis => "this",
            CsomGlobal => "global",
            CsomAuto => "auto",
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid CSObjectMode for 'call' instruction: {:?} at {:08x}",
                    csom,
                    self.addr
                ));
            }
        };
        self.line(&format!("Call {} \"{}\" <{}>", pobj, func_name, num_args))?;
        Ok(())
    }

    fn command_return(&mut self) -> Result<()> {
        let free_stack = self.stream.read_u8()?;
        self.line(&format!("Return {}", free_stack))?;
        Ok(())
    }

    fn command_element(&mut self) -> Result<()> {
        let csvt = self.read_csvt()?;
        match csvt {
            CsvtInteger => {
                let index = self.stream.read_i32()?;
                self.line(&format!("Element {}", index))?;
            }
            CsvtString => {
                let name = self.get_string_literal()?;
                self.line(&format!("Element \"{}\"", name))?;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid CSVariableType for 'element' instruction: {:?} at {:08x}",
                    csvt,
                    self.addr
                ));
            }
        }
        Ok(())
    }

    fn command_element_indirect(&mut self) -> Result<()> {
        self.line("ElementIndirect")?;
        Ok(())
    }

    fn command_operate(&mut self) -> Result<()> {
        let csot = self.read_csot()?;
        match csot {
            CsotNop => {
                self.line("Operate Nop")?;
            }
            CsotAdd => {
                self.line("Operate Add")?;
            }
            CsotSub => {
                self.line("Operate Sub")?;
            }
            CsotMul => {
                self.line("Operate Mul")?;
            }
            CsotDiv => {
                self.line("Operate Div")?;
            }
            CsotMod => {
                self.line("Operate Mod")?;
            }
            CsotAnd => {
                self.line("Operate And")?;
            }
            CsotOr => {
                self.line("Operate Or")?;
            }
            CsotXor => {
                self.line("Operate Xor")?;
            }
            CsotLogicalAnd => {
                self.line("Operate LAnd")?;
            }
            CsotLogicalOr => {
                self.line("Operate LOr")?;
            }
        }
        Ok(())
    }

    fn command_uni_operate(&mut self) -> Result<()> {
        let csuot = self.read_csuot()?;
        match csuot {
            CsuotPlus => {
                self.line("UnaryOperate Plus")?;
            }
            CsuotNegate => {
                self.line("UnaryOperate Negate")?;
            }
            CsuotBitnot => {
                self.line("UnaryOperate BitNot")?;
            }
            CsuotLogicalNot => {
                self.line("UnaryOperate LNot")?;
            }
        }
        Ok(())
    }

    fn command_compare(&mut self) -> Result<()> {
        let csct = self.read_csct()?;
        match csct {
            CsctEqual => {
                self.line("Compare Equal")?;
            }
            CsctNotEqual => {
                self.line("Compare NotEqual")?;
            }
            CsctLessThan => {
                self.line("Compare LessThan")?;
            }
            CsctLessEqual => {
                self.line("Compare LessEqual")?;
            }
            CsctGreaterThan => {
                self.line("Compare GreaterThan")?;
            }
            CsctGreaterEqual => {
                self.line("Compare GreaterEqual")?;
            }
        }
        Ok(())
    }
}
