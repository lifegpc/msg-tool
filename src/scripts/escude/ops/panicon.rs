use super::super::script::ReadParam;
use super::base::CustomOps;
use crate::ext::io::*;
use anyhow::Result;
use int_enum::IntEnum;
use std::collections::HashMap;
use std::io::Seek;

#[repr(u8)]
#[derive(Debug, IntEnum)]
enum PaniconOp {
    End = 0x22,
    Jump,
    Call,
    AutoPlay,
    Frame,
    Text,
    Clear,
    Gap,
    Mes,
    Tlk,
    Menu,
    Select,
    LsfInit,
    LsfSet,
    Cg,
    Em,
    Clr,
    Disp,
    Path,
    Trans,
    BgmPlay,
    BgmStop,
    BgmVolume,
    BgmFx,
    AmbPlay,
    AmbStop,
    AmbVolume,
    AmbFx,
    SePlay,
    SeStop,
    SeWait,
    SeVolume,
    SeFx,
    VocPlay,
    VocStop,
    VocWait,
    VocVolume,
    VocFx,
    Quake,
    Flash,
    Filter,
    Effect,
    Sync,
    Wait,
    Movie,
    Credit,
    Event,
    Scene,
    Title,
    Notice,
    SetPass,
    IsPass,
    AutoSave,
    Place,
    OpenName,
    Name,
    LogNew,
    LogOut,
}

#[derive(Debug)]
pub struct PaniconOps<T: std::fmt::Debug + std::hash::Hash> {
    prev_name: Option<T>,
    menus: HashMap<T, T>,
    last_select: usize,
}

impl<T: std::fmt::Debug + std::hash::Hash> PaniconOps<T> {
    pub fn new() -> Self {
        Self {
            prev_name: None,
            menus: HashMap::new(),
            last_select: 0,
        }
    }
}

use PaniconOp::*;

impl<T> CustomOps<T> for PaniconOps<T>
where
    T: std::fmt::Debug + TryInto<u64> + std::hash::Hash,
{
    fn run<'a>(&mut self, vm: &mut super::super::script::VM<'a, T>, op: u8) -> Result<bool>
    where
        MemReaderRef<'a>: ReadParam<T>,
        T: TryInto<u64>
            + Default
            + Eq
            + Ord
            + Copy
            + std::fmt::Debug
            + std::fmt::Display
            + std::hash::Hash
            + From<u8>
            + std::ops::Neg<Output = T>
            + std::ops::Add<Output = T>
            + std::ops::Sub<Output = T>
            + std::ops::Mul<Output = T>
            + std::ops::Div<Output = T>
            + std::ops::Rem<Output = T>
            + std::ops::Not<Output = T>
            + std::ops::BitAnd<Output = T>
            + std::ops::BitOr<Output = T>
            + std::ops::BitXor<Output = T>
            + std::ops::Shr<Output = T>
            + std::ops::Shl<Output = T>,
        anyhow::Error: From<<T as TryInto<u64>>::Error>,
    {
        if let Ok(op) = PaniconOp::try_from(op) {
            // println!("Running Panicon operation: {:?}", op);
            match op {
                End => vm.skip_n_params(1, false),
                Jump => vm.skip_n_params(1, false),
                Call => vm.skip_n_params(1, false),
                AutoPlay => vm.skip_n_params(1, false),
                Frame => vm.skip_n_params(1, false),
                Text => vm.skip_n_params(2, false),
                Clear => vm.skip_n_params(1, false),
                Gap => vm.skip_n_params(2, false),
                // Handle concat name
                Mes => {
                    let mes = vm.pop_data()?;
                    vm.mess.insert(mes);
                    if let Some(name) = self.prev_name.take() {
                        vm.names.insert(mes, name);
                    }
                    Ok(false)
                }
                Tlk => {
                    let params = vm.read_params(None)?;
                    let name = params
                        .get(0)
                        .cloned()
                        .ok_or(anyhow::anyhow!("Missing name parameter"))?;
                    self.prev_name = Some(name);
                    Ok(false)
                }
                Menu => {
                    let params = vm.read_params(Some(3))?;
                    let id = params[0];
                    let mes = params[1];
                    vm.mess.insert(mes);
                    self.menus.insert(id, mes);
                    Ok(false)
                }
                Select => {
                    if let Some(var) = vm.vars.get_mut(&T::from(131)) {
                        *var = *var + T::from(1);
                        return Ok(false);
                    }
                    let offset = vm.reader.stream_position()? - 1;
                    for _ in self.last_select + 1..self.menus.len() {
                        vm.stack.push(offset);
                        // println!("Pushing offset: {offset:#x} to stack");
                    }
                    vm.vars.insert(T::from(131), T::from(0));
                    vm.skip_n_params(2, false)
                }
                LsfInit => vm.skip_n_params(1, false),
                LsfSet => vm.skip_params(false),
                Cg => vm.skip_params(false),
                Em => vm.skip_n_params(5, false),
                Clr => vm.skip_n_params(1, false),
                Disp => vm.skip_n_params(4, false),
                Path => vm.skip_params(false),
                Trans => Ok(false),
                BgmPlay => vm.skip_n_params(3, false),
                BgmStop => vm.skip_n_params(1, false),
                BgmVolume => vm.skip_n_params(2, false),
                BgmFx => vm.skip_n_params(1, false),
                AmbPlay => vm.skip_n_params(3, false),
                AmbStop => vm.skip_n_params(1, false),
                AmbVolume => vm.skip_n_params(2, false),
                AmbFx => vm.skip_n_params(1, false),
                SePlay => vm.skip_n_params(5, false),
                SeStop => vm.skip_n_params(2, false),
                SeWait => vm.skip_n_params(1, false),
                SeVolume => vm.skip_n_params(3, false),
                SeFx => vm.skip_n_params(1, false),
                VocPlay => vm.skip_n_params(4, false),
                VocStop => vm.skip_n_params(2, false),
                VocWait => vm.skip_n_params(1, false),
                VocVolume => vm.skip_n_params(3, false),
                VocFx => vm.skip_n_params(1, false),
                Quake => vm.skip_n_params(4, false),
                Flash => vm.skip_n_params(2, false),
                Filter => vm.skip_n_params(2, false),
                Effect => vm.skip_n_params(1, false),
                Sync => vm.skip_n_params(2, false),
                Wait => vm.skip_n_params(1, false),
                Movie => vm.skip_n_params(1, false),
                Credit => vm.skip_n_params(1, false),
                Event => vm.skip_n_params(1, false),
                Scene => vm.skip_n_params(1, false),
                Title => {
                    let title = vm.pop_data()?;
                    vm.mess.insert(title);
                    Ok(false)
                }
                Notice => {
                    let notice = vm.pop_data()?;
                    vm.mess.insert(notice);
                    vm.skip_n_params(2, false)
                }
                SetPass => vm.skip_n_params(2, false),
                IsPass => vm.skip_n_params(1, false),
                AutoSave => Ok(false),
                Place => vm.skip_n_params(1, false),
                OpenName => vm.skip_n_params(1, false),
                Name => {
                    eprintln!("Name operation.");
                    let mes = vm.pop_data()?;
                    let name = vm.pop_data()?;
                    vm.mess.insert(mes);
                    vm.names.insert(mes, name);
                    Ok(false)
                }
                LogNew => vm.skip_n_params(1, false),
                LogOut => vm.skip_params(false),
            }
        } else {
            // return Err(anyhow::anyhow!("Unknown Panicon operation: {op:#02x}"));
            Ok(false)
        }
    }
}
