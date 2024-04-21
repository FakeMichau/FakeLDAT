use std::{thread::sleep, time::Duration};

use clap::Parser;
use fakeldat_lib::{self, serialport, Error, FakeLDAT, Report};

#[derive(Parser)]
struct Args {
    /// Name of the port, i.e. /dev/ttyACM0 on Linux or COM1 on Windows
    #[arg(short, long)]
    port: String,
    /// Set device poll rate
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Get value of a setting
    #[command(subcommand)]
    Get(SettingGet),
    /// Set a setting
    #[command(subcommand)]
    Set(SettingSet),
    /// Set a setting
    ManualTrigger
}

#[derive(clap::Subcommand)]
enum SettingSet {
    /// Set Poll rate
    PollRate(PollRate),
    /// Set Report mode
    ReportMode(ReportModeS),
    /// Set Threshold
    Threshold(Threshold),
    /// Set Action key
    Action(ActionModeS),
}

#[derive(clap::Subcommand)]
enum SettingGet {
    /// Get Poll rate
    PollRate,
    /// Get ReportMode
    ReportMode,
    /// Get Threshold
    Threshold,
    // Get Action key
    Action,
}

#[derive(clap::Args)]
struct PollRate {
    value: u16,
}

#[derive(clap::Args)]
struct Threshold {
    value: i16,
}

#[derive(clap::Args)]
struct ReportModeS {
    value: ReportMode,
}

#[derive(Clone, clap::ValueEnum)]
enum ReportMode {
    Raw,
    Summary,
    Combined,
}

impl From<ReportMode> for fakeldat_lib::ReportMode {
    fn from(value: ReportMode) -> Self {
        match value {
            ReportMode::Raw => Self::Raw,
            ReportMode::Summary => Self::Summary,
            ReportMode::Combined => Self::Combined,
        }
    }
}

#[derive(clap::Args)]
struct ActionModeS {
    action_mode: ActionMode,
    key: Key,
}

impl From<ActionModeS> for fakeldat_lib::ActionMode {
    fn from(value: ActionModeS) -> Self {
        match value.action_mode {
            ActionMode::Mouse => {
                Self::Mouse((value.key as u8).try_into().expect("Invalid mouse button"))
            }
            ActionMode::Keyboard => {
                Self::Keyboard((value.key as u8).try_into().expect("Invalid keyboard key"))
            }
        }
    }
}

#[derive(Clone, clap::ValueEnum)]
#[repr(u8)]
enum Key {
    Left = 1,
    Right = 2,
    Middle = 4,
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

#[derive(Clone, clap::ValueEnum)]
enum ActionMode {
    Mouse,
    Keyboard,
}

fn main() {
    if let Some(err) = handle_fakeldat().err() {
        match err {
            Error::WrongChecksum(_, _, _) | Error::ReadTooLittleData => unreachable!(), // Those should be internal
            Error::InvalidSetting(command, buf) => {
                eprintln!("Invalid setting for {command}: {:x} {:x}", buf[0], buf[1]);
            }
            Error::InvalidCommand(command_id) => eprintln!("Invalid command id: {command_id}"),
            Error::SendCommandFail => eprintln!("Issue with sending a command"),
            Error::IOError(io_error) => eprintln!("Issue with saving a file: {io_error}"),
            Error::InvalidEnumConverion => eprintln!("TryFrom enum conversion error"),
            Error::PortFail(serialport_error) => {
                eprintln!("Port fail: {}", serialport_error.description);
            }
        }
    }
}

fn handle_fakeldat() -> Result<(), Error> {
    let args = Args::parse();

    let port = serialport::new(args.port, 115_200)
        .timeout(Duration::from_secs(100_000))
        .open()?;

    let mut fakeldat = FakeLDAT::create(port)?;

    if let Some(command) = args.command {
        match command {
            Command::Get(setting) => match setting {
                SettingGet::PollRate => fakeldat.get_poll_rate(),
                SettingGet::ReportMode => fakeldat.get_report_mode(),
                SettingGet::Threshold => fakeldat.get_threshold(),
                SettingGet::Action => fakeldat.get_action(),
            },
            Command::Set(setting) => match setting {
                SettingSet::PollRate(poll_rate) => fakeldat.set_poll_rate(poll_rate.value),
                SettingSet::ReportMode(report_mode) => {
                    fakeldat.set_report_mode(report_mode.value.into())
                }
                SettingSet::Threshold(threshold) => fakeldat.set_threshold(threshold.value),
                SettingSet::Action(action) => fakeldat.set_action(action.into()),
            },
            Command::ManualTrigger => {
                return fakeldat.manual_trigger();
            }
        }?;
        loop {
            fakeldat.poll_bulk_data()?;
            if let Some(reports) = fakeldat.take_report_buffer() {
                for report in reports {
                    match report {
                        Report::PollRate(poll_rate) => {
                            println!("Poll rate: {poll_rate}");
                            return Ok(());
                        }
                        Report::ReportMode(report_mode) => {
                            println!("Report mode: {report_mode}");
                            return Ok(());
                        }
                        Report::Threshold(threshold) => {
                            println!("Threshold: {threshold}");
                            return Ok(());
                        }
                        Report::Action(action) => {
                            match action {
                                fakeldat_lib::ActionMode::Mouse(button) => {
                                    println!("Action: Mouse, {button}");
                                }
                                fakeldat_lib::ActionMode::Keyboard(key) => {
                                    println!("Action: Keyboard, {key}");
                                }
                            };
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
            sleep(Duration::from_millis(50));
        }
    } else {
        loop {
            fakeldat.poll_bulk_data()?;
            if let Some(reports) = fakeldat.take_report_buffer() {
                for report in reports {
                    match report {
                        Report::Raw(raw_report) => {
                            println!(
                                "{}, {}, {}",
                                raw_report.timestamp, raw_report.brightness, raw_report.trigger
                            );
                        }
                        Report::Summary(summary_report) => {
                            println!("{}, {}", summary_report.delay, summary_report.threshold);
                        }
                        _ => {}
                    }
                }
            }
            sleep(Duration::from_millis(50));
        }
    }
}
