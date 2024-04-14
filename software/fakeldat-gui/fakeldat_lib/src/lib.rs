use std::{fmt::Display, mem::take};

pub use serialport;
use serialport::SerialPort;
use std::io::Read;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    // command with the error, expected checksum, calculated checksum
    WrongChecksum(Command, u8, u8),
    // command and the invalid settings
    InvalidSetting(Command, [u8; 2]),
    // value of the command received
    InvalidCommand(u8),
    PortFail(serialport::Error),
    ReadTooLittleData,
    SendCommandFail,
    IOError(std::io::Error),
    InvalidEnumConverion,
}

impl From<serialport::Error> for Error {
    fn from(value: serialport::Error) -> Self {
        Self::PortFail(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

macro_rules! create_try_from {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])* $vname:ident $(= $val:expr)?,)*
    }) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $vname $(= $val)?,)*
        }

        impl std::convert::TryFrom<u8> for $name {
            type Error = Error;

            fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
                match v {
                    $(x if x == $name::$vname as u8 => Ok($name::$vname),)*
                    _ => Err(Error::InvalidEnumConverion),
                }
            }
        }
    }
}

create_try_from! {
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
    pub enum Command {
        SetPollRate = 0x01,
        GetPollRate = 0x21,
        SetReportMode = 0x02,
        GetReportMode = 0x22,
        SetThreshold = 0x03,
        GetThreshold = 0x23,
        SetAction = 0x04,
        GetAction = 0x24,
        ManualTrigger = 0x1F,
        ReportRaw = 0x41,
        ReportSummary = 0x42,
    }
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::ReportRaw => "Raw",
                Self::ReportSummary => "Summary",
                Self::SetPollRate => "Set poll rate",
                Self::GetPollRate => "Get poll rate",
                Self::SetReportMode => "Set report mode",
                Self::GetReportMode => "Get report mode",
                Self::SetThreshold => "Set threshold",
                Self::GetThreshold => "Get threshold",
                Self::SetAction => "Set action",
                Self::GetAction => "Get action",
                Self::ManualTrigger => "Manual trigger",
            }
        )
    }
}

create_try_from! {
    #[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
    pub enum ReportMode {
        Raw,
        Summary,
        Combined,
    }
}

impl std::fmt::Display for ReportMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Raw => "Raw",
                Self::Summary => "Summary",
                Self::Combined => "Combined",
            }
        )
    }
}

create_try_from! {
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
    pub enum KeyboardKey {
        A = b'a',
        B = b'b',
        C = b'c',
        D = b'd',
        E = b'e',
        F = b'f',
        G = b'g',
        H = b'h',
        I = b'i',
        J = b'j',
        K = b'k',
        L = b'l',
        M = b'm',
        N = b'n',
        O = b'o',
        P = b'p',
        Q = b'q',
        R = b'r',
        S = b's',
        T = b't',
        U = b'u',
        V = b'v',
        W = b'w',
        X = b'x',
        Y = b'y',
        Z = b'z',
    }
}

impl KeyboardKey {
    pub const ALL: [Self; 26] = [
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::H,
        Self::I,
        Self::J,
        Self::K,
        Self::L,
        Self::M,
        Self::N,
        Self::O,
        Self::P,
        Self::Q,
        Self::R,
        Self::S,
        Self::T,
        Self::U,
        Self::V,
        Self::W,
        Self::X,
        Self::Y,
        Self::Z,
    ];
}

impl Display for KeyboardKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", (*self as u8 as char).to_uppercase())
    }
}

create_try_from! {
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
    pub enum MouseButton {
        Left = 1,
        Right = 2,
        Middle = 4,
    }
}

impl MouseButton {
    pub const ALL: [Self; 3] = [Self::Left, Self::Right, Self::Middle];
}

impl std::fmt::Display for MouseButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Left => "Left",
                Self::Right => "Right",
                Self::Middle => "Middle",
            }
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
pub enum ActionMode {
    Mouse(MouseButton),
    Keyboard(KeyboardKey),
}

impl ActionMode {
    const fn get_key(self) -> u8 {
        match self {
            Self::Mouse(button) => button as u8,
            Self::Keyboard(key) => key as u8,
        }
    }

    pub fn try_from(mode: u8, key: u8) -> Result<Self> {
        match mode {
            0 => Ok(Self::Mouse(MouseButton::try_from(key)?)),
            1 => Ok(Self::Keyboard(KeyboardKey::try_from(key)?)),
            _ => Err(Error::InvalidSetting(Command::SetAction, [mode, key])),
        }
    }
}

impl From<ActionMode> for u8 {
    fn from(value: ActionMode) -> Self {
        match value {
            ActionMode::Mouse(_) => 0,
            ActionMode::Keyboard(_) => 1,
        }
    }
}

pub enum Report {
    Raw(RawReport),
    Summary(SummaryReport),
    PollRate(u16),
    ReportMode(ReportMode),
    Threshold(i16),
    Action(ActionMode), // action and key
    ManualTrigger,
}

pub struct RawReport {
    pub timestamp: u64,
    pub brightness: u16,
    pub trigger: bool,
}

pub struct SummaryReport {
    pub delay: u64,
    pub threshold: u16,
}

pub fn sum_slice(data: &[u8]) -> u8 {
    data.iter().fold(0, |acc, &x| acc.wrapping_add(x))
}

pub struct FakeLDAT {
    report_buffer: Option<Vec<Report>>,
    read: Box<dyn SerialPort>,
    port: Box<dyn SerialPort>,
}

impl FakeLDAT {
    pub fn create(mut port: Box<dyn SerialPort>) -> Result<Self> {
        // TODO: create port here given some unique characteristic
        port.write_data_terminal_ready(true)?;
        Ok(Self {
            report_buffer: Some(Vec::new()),
            read: port.try_clone()?,
            port,
        })
    }
    fn send_command<T: std::io::Write>(
        command: Command,
        args: [u8; 2],
        port: &mut T,
    ) -> Result<()> {
        let mut buf = [0; 16];
        buf[0] = command as u8;
        buf[1] = args[0];
        buf[2] = args[1];
        // 3 - 14 unused
        buf[15] = sum_slice(&buf[..3]);
        port.write_all(&buf).map_err(|_| Error::SendCommandFail)
    }

    pub fn set_poll_rate(&mut self, pollrate_hz: u16) -> Result<()> {
        Self::send_command(
            Command::SetPollRate,
            pollrate_hz.to_le_bytes(),
            &mut self.port,
        )
    }
    pub fn set_threshold(&mut self, threshold: i16) -> Result<()> {
        Self::send_command(
            Command::SetThreshold,
            threshold.to_le_bytes(),
            &mut self.port,
        )
    }
    pub fn set_report_mode(&mut self, report_mode: ReportMode) -> Result<()> {
        Self::send_command(
            Command::SetReportMode,
            [report_mode as u8, 0],
            &mut self.port,
        )
    }
    pub fn set_action(&mut self, action_mode: ActionMode) -> Result<()> {
        Self::send_command(
            Command::SetAction,
            [action_mode.into(), action_mode.get_key()],
            &mut self.port,
        )
    }

    pub fn get_poll_rate(&mut self) -> Result<()> {
        Self::send_command(Command::GetPollRate, [0, 0], &mut self.port)
    }
    pub fn get_threshold(&mut self) -> Result<()> {
        Self::send_command(Command::GetThreshold, [0, 0], &mut self.port)
    }
    pub fn get_report_mode(&mut self) -> Result<()> {
        Self::send_command(Command::GetReportMode, [0, 0], &mut self.port)
    }
    pub fn get_action(&mut self) -> Result<()> {
        Self::send_command(Command::GetAction, [0, 0], &mut self.port)
    }

    pub fn manual_trigger(&mut self) -> Result<()> {
        Self::send_command(Command::ManualTrigger, [0, 0], &mut self.port)
    }

    #[allow(clippy::too_many_lines)]
    // This will block
    fn poll_data(&mut self) -> Result<Report> {
        if self.port.bytes_to_read()? < 16 {
            return Err(Error::ReadTooLittleData);
        }

        let mut buf = [0u8; 16];
        self.read.read_exact(&mut buf)?;

        let Ok(command) = buf[0].try_into() else {
            return Err(Error::InvalidCommand(buf[0]));
        };

        let calculated_checksum: u8 = sum_slice(&buf[..=14]);
        let received_checksum = buf[15];
        if received_checksum != calculated_checksum {
            return Err(Error::WrongChecksum(
                command,
                received_checksum,
                calculated_checksum,
            ));
        }
        let settings_buffer: [u8; 2] = buf[1..=2].try_into().unwrap();

        match command {
            Command::ReportRaw => Ok(Report::Raw(RawReport {
                timestamp: u64::from_le_bytes(buf[1..=8].try_into().unwrap()),
                brightness: u16::from_le_bytes(buf[9..=10].try_into().unwrap()),
                trigger: buf[11] == 1,
            })),
            Command::ReportSummary => Ok(Report::Summary(SummaryReport {
                delay: u64::from_le_bytes(buf[1..=8].try_into().unwrap()),
                threshold: u16::from_le_bytes(buf[9..=10].try_into().unwrap()),
            })),
            Command::GetPollRate | Command::SetPollRate => {
                Ok(Report::PollRate(u16::from_le_bytes(settings_buffer)))
            }
            Command::GetReportMode | Command::SetReportMode => {
                ReportMode::try_from(settings_buffer[0]).map_or_else(
                    |_| Err(Error::InvalidSetting(command, settings_buffer)),
                    |report_mode| Ok(Report::ReportMode(report_mode)),
                )
            }
            Command::GetThreshold | Command::SetThreshold => {
                Ok(Report::Threshold(i16::from_le_bytes(settings_buffer)))
            }
            Command::GetAction | Command::SetAction => {
                ActionMode::try_from(settings_buffer[0], settings_buffer[1]).map_or_else(
                    |_| Err(Error::InvalidSetting(command, settings_buffer)),
                    |action_mode| Ok(Report::Action(action_mode)),
                )
            }
            Command::ManualTrigger => Ok(Report::ManualTrigger),
        }
    }

    pub fn take_report_buffer(&mut self) -> Option<Vec<Report>> {
        if self.report_buffer.is_some() {
            take(&mut self.report_buffer)
        } else {
            None
        }
    }

    pub fn poll_bulk_data(&mut self) -> Result<()> {
        // TODO: what if serial buffer gets full in the meantime
        let mut read_next = true;
        while read_next {
            match self.poll_data() {
                Ok(report) => {
                    if let Some(ref mut report_buffer) = self.report_buffer {
                        report_buffer.push(report);
                    } else {
                        self.report_buffer = Some(vec![report]);
                    }
                }
                Err(why) => match why {
                    Error::ReadTooLittleData => read_next = false,
                    Error::WrongChecksum(a, b, c) => {
                        println!("Wrong checksum: {a}, {b}, {c}");
                        self.port.clear(serialport::ClearBuffer::Input)?;
                    }
                    why => return Result::Err(why),
                },
            }
        }
        Ok(())
    }
}
