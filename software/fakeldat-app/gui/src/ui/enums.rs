use fakeldat_lib::{KeyboardKey, MouseButton, ReportMode};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    RecordStart,
    RecordStop,
    Clear,
    GraphToggle,
    ManualTrigger,
    PollRateChanged(PollRate),
    ReportModeChanged(ReportMode),
    ActionModeChanged(ActionType),
    ActionKeyChanged(u8),
    ThresholdChanged(i16),
    ThresholdReleased,
}

#[derive(Default)]
pub struct ActionKey {
    pub mouse: Option<MouseButton>,
    pub keyboard: Option<KeyboardKey>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Mouse,
    Keyboard,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mouse => write!(f, "Mouse"),
            Self::Keyboard => write!(f, "Keyboard"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollRate {
    _500,
    _1000,
    _2000,
    _4000,
    _8000,
    _16000,
    _32000,
}

impl PollRate {
    pub const ALL: [Self; 7] = [
        Self::_500,
        Self::_1000,
        Self::_2000,
        Self::_4000,
        Self::_8000,
        Self::_16000,
        Self::_32000,
    ];
}

impl From<PollRate> for u16 {
    fn from(val: PollRate) -> Self {
        match val {
            PollRate::_500 => 500,
            PollRate::_1000 => 1000,
            PollRate::_2000 => 2000,
            PollRate::_4000 => 4000,
            PollRate::_8000 => 8000,
            PollRate::_16000 => 16000,
            PollRate::_32000 => 32000,
        }
    }
}

impl std::fmt::Display for PollRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", std::convert::Into::<u16>::into(*self))
    }
}

impl From<u16> for PollRate {
    fn from(value: u16) -> Self {
        match value {
            x if x > 24000 => Self::_32000,
            x if x > 12000 => Self::_16000,
            x if x > 6000 => Self::_8000,
            x if x > 3000 => Self::_4000,
            x if x > 1500 => Self::_2000,
            x if x > 750 => Self::_1000,
            _ => Self::_500,
        }
    }
}
