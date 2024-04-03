use std::{io::Bytes, iter::Peekable, mem::take};

pub use serialport;
use serialport::SerialPort;
use std::io::Read;

#[derive(Debug, Clone)]
pub enum Error {
    // command with the error, expected checksum, calculated checksum
    WrongChecksum(Command, u8, u8),
    // command and the invalid settings
    InvalidSetting(Command, [u8; 2]),
    // value of the command received
    InvalidCommand(u8),
    // value of the command and settings received
    Unimplemented(u8, [u8; 2]),
    PortFail,
    ReadFail,
    ReadTooLittleData,
}

macro_rules! convert_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])* $vname:ident $(= $val:expr)?,)*
    }) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $vname $(= $val)?,)*
        }

        impl std::convert::TryFrom<u8> for $name {
            type Error = ();

            fn try_from(v: u8) -> Result<Self, Self::Error> {
                match v {
                    $(x if x == $name::$vname as u8 => Ok($name::$vname),)*
                    _ => Err(()),
                }
            }
        }
    }
}

convert_enum! {
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
        match self {
            Self::ReportRaw => write!(f, "Raw"),
            Self::ReportSummary => write!(f, "Summary"),
            _ => todo!(),
        }
    }
}

convert_enum! {
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

convert_enum! {
    #[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
    pub enum ActionMode {
        Mouse,
        Keyboard,
    }
}

impl std::fmt::Display for ActionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mouse => write!(f, "Mouse"),
            Self::Keyboard => write!(f, "Keyboard"),
        }
    }
}

pub enum Report {
    Raw(RawReport),
    Summary(SummaryReport),
    PollRate(u16),
    ReportMode(ReportMode),
    Threshold(i16),
    Action(ActionMode, u8), // action and key
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
    read_iter: Peekable<Bytes<Box<dyn SerialPort>>>,
    port: Box<dyn SerialPort>,
}

impl FakeLDAT {
    pub fn create(mut port: Box<dyn SerialPort>) -> Result<Self, Error> {
        // TODO: create port here given some unique characteristic
        port.write_data_terminal_ready(true);
        Ok(Self {
            report_buffer: Some(Vec::new()),
            read_iter: port
                .try_clone()
                .map_err(|_| Error::PortFail)?
                .bytes()
                .peekable(),
            port,
        })
    }
    fn send_command<T: std::io::Write>(
        command: Command,
        args: [u8; 2],
        port: &mut T,
    ) -> std::io::Result<()> {
        let mut buf = [0; 4];
        buf[0] = command as u8;
        buf[1] = args[0];
        buf[2] = args[1];
        buf[3] = sum_slice(&buf[..3]);
        port.write_all(&buf)
    }

    pub fn set_poll_rate(&mut self, pollrate_hz: u16) {
        Self::send_command(
            Command::SetPollRate,
            pollrate_hz.to_le_bytes(),
            &mut self.port,
        )
        .expect("Set Poll Rate");
    }
    pub fn set_threshold(&mut self, threshold: i16) {
        Self::send_command(
            Command::SetThreshold,
            threshold.to_le_bytes(),
            &mut self.port,
        )
        .expect("Set Threshold");
    }
    pub fn set_report_mode(&mut self, report_mode: ReportMode) {
        Self::send_command(
            Command::SetReportMode,
            [report_mode as u8, 0],
            &mut self.port,
        )
        .expect("Set Report Mode");
    }
    pub fn set_action(&mut self, action_mode: ActionMode, key: u8) {
        Self::send_command(
            Command::SetAction,
            [action_mode as u8, key],
            &mut self.port,
        )
        .expect("Set Action");
    }

    pub fn get_poll_rate(&mut self) {
        Self::send_command(Command::GetPollRate, [0, 0], &mut self.port).expect("Get Poll Rate");
    }
    pub fn get_threshold(&mut self) {
        Self::send_command(Command::GetThreshold, [0, 0], &mut self.port).expect("Get Threshold");
    }
    pub fn get_report_mode(&mut self) {
        Self::send_command(Command::GetReportMode, [0, 0], &mut self.port)
            .expect("Get Report Mode");
    }
    pub fn get_action(&mut self) {
        Self::send_command(Command::GetAction, [0, 0], &mut self.port).expect("Get Action");
    }

    #[allow(clippy::too_many_lines)]
    // This will block
    fn poll_data(&mut self) -> Result<Report, Error> {
        let mut command_buffer = [0u8; 1];
        if self.port.bytes_to_read().unwrap() == 0 {
            // needed because otherwise peek will block
            return Err(Error::ReadTooLittleData);
        }
        if let Some(command_peek) = self.read_iter.peek() {
            let command = command_peek.as_ref().expect("Command peek");
            // 12 and 3 instead of 13 and 4 because peek reads one byte
            if command == &(Command::ReportRaw as u8) || command == &(Command::ReportSummary as u8) {
                if self.port.bytes_to_read().unwrap_or_default() < 12 {
                    return Err(Error::ReadTooLittleData);
                }
            } else if self.port.bytes_to_read().unwrap_or_default() < 3 {
                return Err(Error::ReadTooLittleData);
            }
        } else {
            return Err(Error::ReadTooLittleData);
        }
        command_buffer[0] = self
            .read_iter
            .next()
            .expect("Command")
            .map_err(|_| Error::ReadFail)?;
        let Ok(command) = command_buffer[0].try_into() else {
            return Err(Error::InvalidCommand(command_buffer[0]));
        };

        // Reports are 13 bytes, settings are 4 bytes
        if command == Command::ReportRaw || command == Command::ReportSummary {
            let mut buf = [0u8; 12];
            for i in 0..12 {
                buf[i] = self
                    .read_iter
                    .next()
                    .expect("Data")
                    .map_err(|_| Error::ReadFail)?;
            }
            let calculated_checksum: u8 = sum_slice(&buf[..=10]).wrapping_add(command_buffer[0]);
            if buf[11] == calculated_checksum {
                let first = u64::from_le_bytes(buf[..=7].try_into().unwrap());
                let second = u16::from_le_bytes(buf[8..=9].try_into().unwrap());
                Ok(match command {
                    Command::ReportRaw => Report::Raw(RawReport {
                        timestamp: first,
                        brightness: second,
                        trigger: buf[10] == 1,
                    }),
                    Command::ReportSummary => Report::Summary(SummaryReport {
                        delay: first,
                        threshold: second,
                    }),
                    _ => unreachable!(),
                })
            } else {
                Err(Error::WrongChecksum(command, buf[11], calculated_checksum))
            }
        } else {
            let mut settings_buffer = [0u8; 2];
            let mut checksum_buffer = [0u8; 1];
            for i in 0..2 {
                settings_buffer[i] = self
                    .read_iter
                    .next()
                    .expect("Data")
                    .map_err(|_| Error::ReadFail)?;
            }
            checksum_buffer[0] = self
                .read_iter
                .next()
                .expect("Data")
                .map_err(|_| Error::ReadFail)?;
            let calculated_checksum: u8 =
                sum_slice(&[command_buffer[0], settings_buffer[0], settings_buffer[1]]);
            if checksum_buffer[0] != calculated_checksum {
                return Err(Error::WrongChecksum(
                    command,
                    checksum_buffer[0],
                    calculated_checksum,
                ));
            }
            match command {
                Command::GetPollRate | Command::SetPollRate => {
                    Ok(Report::PollRate(u16::from_le_bytes(settings_buffer)))
                }
                Command::GetReportMode | Command::SetReportMode => {
                    let Ok(report_mode) = ReportMode::try_from(settings_buffer[0]) else {
                        return Err(Error::InvalidSetting(command, settings_buffer));
                    };
                    Ok(Report::ReportMode(report_mode))
                }
                Command::GetThreshold | Command::SetThreshold => {
                    Ok(Report::Threshold(i16::from_le_bytes(settings_buffer)))
                }
                Command::GetAction | Command::SetAction => {
                    let Ok(action_mode) = ActionMode::try_from(settings_buffer[0]) else {
                        return Err(Error::InvalidSetting(command, settings_buffer));
                    };
                    Ok(Report::Action(action_mode, settings_buffer[1]))
                }
                _ => Err(Error::Unimplemented(command as u8, settings_buffer)),
            }
        }
    }

    pub fn take_report_buffer(&mut self) -> Option<Vec<Report>> {
        if self.report_buffer.is_some() {
            take(&mut self.report_buffer)
        } else {
            None
        }
    }

    pub fn poll_bulk_data(&mut self) {
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
                        self.port.clear(serialport::ClearBuffer::Input);
                    }
                    _ => todo!(),
                },
            }
        }
    }
}
