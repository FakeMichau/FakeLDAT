mod enums;
use chrono::{DateTime, Utc};
#[allow(clippy::wildcard_imports)]
use enums::*;
use fakeldat_lib::{
    serialport::{self, SerialPort},
    ActionMode, Error, FakeLDAT, KeyboardKey, MouseButton, RawReport, Report, ReportMode,
    SummaryReport,
};
use iced::widget::{
    button, column, container, pick_list, radio, row, scrollable, slider, text, Container, Rule,
    Scrollable, Space,
};
use iced::{Alignment, Length, Subscription, Theme};
use plotters::coord::Shift;
use plotters::element::Rectangle;
use plotters::series::LineSeries;
use plotters::style::{Color, BLUE, GREEN, RED, WHITE};
use plotters_iced::{Chart, ChartBuilder, ChartWidget, DrawingArea, DrawingBackend};
use rfd::FileDialog;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::Duration;
use std::{cmp::Ordering, process::exit, thread::sleep};

pub struct UI {
    fakeldat: FakeLDAT,
    theme: Theme,
    selected_pollrate: PollRate,
    selected_reportmode: ReportMode,
    selected_action_type: ActionType,
    selected_action_key: ActionKey,
    threshold: i16,
    show_graph: bool,
    record_file: Option<File>,
    raw_data: VecDeque<RawReport>,    // data refactor?
    summary_data: Vec<SummaryReport>, // TODO: old data is not being removed
    macro_timestamps: Vec<u64>,       // TODO: old data is not being removed
    trigger_timestamps: Vec<u64>,     // TODO: old data is not being removed
    init_process: u8,
    forced_tick_rate: Option<u16>,
}

impl Default for UI {
    fn default() -> Self {
        let port;
        let mut error_count = 0;
        loop {
            if let Ok(new_port) = Self::get_port() {
                port = new_port;
                break;
            }
            eprintln!("Can't find device");
            error_count += 1;
            if error_count == 30 {
                exit(1)
            }
            sleep(Duration::from_secs(2));
        }
        Self {
            fakeldat: FakeLDAT::create(port).expect("Couldn't create FakeLDAT"),
            theme: Theme::Dark,
            selected_pollrate: PollRate::_2000,
            selected_reportmode: ReportMode::Raw,
            selected_action_type: ActionType::Mouse,
            selected_action_key: ActionKey::default(),
            threshold: 150,
            show_graph: true,
            record_file: None,
            raw_data: VecDeque::new(),
            summary_data: Vec::new(),
            macro_timestamps: Vec::new(),
            trigger_timestamps: Vec::new(),
            init_process: 0,
            forced_tick_rate: None,
        }
    }
}

impl UI {
    #[allow(clippy::needless_pass_by_value)]
    pub fn update(&mut self, message: Message) {
        if let Err(why) = self.update_with_error(message) {
            match why {
                Error::WrongChecksum(_, _, _) | Error::ReadTooLittleData => unreachable!(), // Those should be internal
                Error::InvalidSetting(command, buf) => {
                    eprintln!("Invalid setting for {command}: {:x} {:x}", buf[0], buf[1]);
                }
                Error::InvalidCommand(command_id) => eprintln!("Invalid command id: {command_id}"),
                Error::PortFail(serialport_error) => {
                    match serialport_error.kind {
                        serialport::ErrorKind::NoDevice | serialport::ErrorKind::Unknown => {
                            self.forced_tick_rate = Some(1);
                            // This allows the UI to not freeze
                            if Self::get_port().is_ok() {
                                *self = Self::default();
                            }
                        }
                        _ => todo!(),
                    };
                    eprintln!("Port fail: {}", serialport_error.description);
                }
                Error::SendCommandFail => eprintln!("Issue with sending a command"),
                Error::IOError(io_error) => eprintln!("Issue with saving a file: {io_error}"),
                Error::InvalidEnumConverion => eprintln!("TryFrom enum conversion error"),
            }
        };
    }

    pub fn view(&self) -> iced::Element<Message> {
        let spacer = Rule::horizontal(1);
        let main_stack = column![
            self.draw_graph(),
            self.draw_buttons(),
            spacer,
            self.draw_rate_selection(),
            self.draw_mode_selection(),
            self.draw_action_selection(),
            self.threshold_selection(),
        ];

        container(main_stack)
            .center_x()
            .center_y()
            .padding(20)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }

    #[allow(clippy::needless_pass_by_value)]
    fn update_with_error(&mut self, message: Message) -> Result<(), Error> {
        match message {
            Message::Tick => {
                self.tick()?;
            }
            Message::RecordStart => {
                let now: DateTime<Utc> = Utc::now();
                let path = FileDialog::new()
                    .set_directory("/")
                    .pick_folder()
                    .map(|record_dir| {
                        record_dir.join(format!(
                            "{}_report {}.csv",
                            self.selected_reportmode.to_string().to_lowercase(),
                            now.format("%d-%m-%Y %H.%M.%S")
                        ))
                    });
                if let Some(path) = path {
                    self.record_file = Some(
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .map_err(Error::IOError)?,
                    );
                }
            }
            Message::RecordStop => self.record_file = None,
            Message::Clear => {
                self.raw_data = vec![].into();
                self.summary_data = vec![];
            }
            Message::GraphToggle => self.show_graph = !self.show_graph,
            Message::ManualTrigger => {
                self.fakeldat.manual_trigger()?;
            }
            Message::PollRateChanged(pollrate) => {
                self.fakeldat.set_poll_rate(pollrate.into())?;
            }
            Message::ReportModeChanged(report_mode) => {
                self.fakeldat.set_report_mode(report_mode)?;
                self.record_file = None;
            }
            Message::ActionModeChanged(action_type) => {
                self.selected_action_type = action_type;
                let key_option = match action_type {
                    ActionType::Mouse => self.selected_action_key.mouse.map(|v| v as u8),
                    ActionType::Keyboard => self.selected_action_key.keyboard.map(|v| v as u8),
                };
                if let Some(key) = key_option {
                    let action_mode = ActionMode::try_from(self.selected_action_type as u8, key)?;
                    self.fakeldat.set_action(action_mode)?;
                }
            }
            Message::ActionKeyChanged(key) => {
                let action_mode = ActionMode::try_from(self.selected_action_type as u8, key)?;
                self.fakeldat.set_action(action_mode)?;
            }
            Message::ThresholdChanged(threshold) => self.threshold = threshold,
            Message::ThresholdReleased => {
                self.fakeldat.set_threshold(self.threshold)?;
            }
        }
        Ok(())
    }

    // Only for polling data, window refresh is separate
    fn tick(&mut self) -> Result<(), Error> {
        self.fakeldat.poll_bulk_data()?;
        if self.init_process < 10 {
            _ = self.fakeldat.take_report_buffer();
        }
        if let Some(reports) = self.fakeldat.take_report_buffer() {
            let mut record_buffer = vec![];
            for report in reports {
                match report {
                    Report::Raw(raw_report) => {
                        if let Some(last_record) = self.raw_data.back() {
                            if !last_record.trigger && raw_report.trigger {
                                self.trigger_timestamps.push(raw_report.timestamp);
                            }
                        }
                        record_buffer.push(format!(
                            "{},{},{}",
                            raw_report.timestamp,
                            raw_report.brightness,
                            u8::from(raw_report.trigger)
                        ));
                        self.push_data(raw_report);
                    }
                    Report::Summary(summary_report) => {
                        record_buffer.push(format!(
                            "{},{}",
                            summary_report.delay, summary_report.threshold
                        ));
                        self.summary_data.push(summary_report);
                    }
                    Report::PollRate(pollrate) => {
                        self.selected_pollrate = pollrate.try_into().expect("Wrong poll rate");
                    }
                    Report::Action(action_mode) => match action_mode {
                        ActionMode::Mouse(button) => {
                            self.selected_action_type = ActionType::Mouse;
                            self.selected_action_key.mouse = Some(button);
                        }
                        ActionMode::Keyboard(keyboard_key) => {
                            self.selected_action_type = ActionType::Keyboard;
                            self.selected_action_key.keyboard = Some(keyboard_key);
                        }
                    },
                    Report::ReportMode(report_mode) => {
                        self.selected_reportmode = report_mode;
                    }
                    Report::Threshold(threshold) => {
                        self.threshold = threshold;
                    }
                    Report::MacroTrigger(timestamp) => self.macro_timestamps.push(timestamp),
                    Report::ManualTrigger => { /* Manual trigger successful */ }
                }
            }
            if let Some(ref mut record_file) = &mut self.record_file {
                let mut data = record_buffer.join("\n");
                data.push('\n');
                record_file
                    .write_all(data.as_ref())
                    .map_err(Error::IOError)?;
            }
        }
        if self.init_process <= 10 {
            self.init_process += 1;
        }
        if self.init_process == 10 {
            self.fakeldat.get_action()?;
            self.fakeldat.get_poll_rate()?;
            self.fakeldat.get_threshold()?;
            self.fakeldat.get_report_mode()?;
        };
        Ok(())
    }

    fn draw_graph(&self) -> iced::Element<Message> {
        let graph_raw = if self.show_graph
            && (self.selected_reportmode == ReportMode::Raw
                || self.selected_reportmode == ReportMode::Combined)
        {
            container(
                ChartWidget::new(self)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
        } else if !self.show_graph {
            container(Space::new(Length::Fill, Length::Fill))
        } else {
            // When showing the other graph
            container(Space::new(Length::Shrink, Length::Shrink))
        };
        let graph_summary = if self.show_graph
            && (self.selected_reportmode == ReportMode::Summary
                || self.selected_reportmode == ReportMode::Combined)
        {
            container(
                Scrollable::with_direction(
                    text(
                        self.summary_data
                            .iter()
                            .map(|summary| format!("{}, {}", summary.delay, summary.threshold))
                            .collect::<Vec<String>>()
                            .join("\n"),
                    )
                    .vertical_alignment(iced::alignment::Vertical::Top),
                    scrollable::Direction::Vertical(
                        scrollable::Properties::new().alignment(scrollable::Alignment::End),
                    ),
                )
                .width(Length::Fill)
                .height(Length::Fill),
            )
        } else if !self.show_graph {
            container(Space::new(Length::Fill, Length::Fill))
        } else {
            // When showing the other graph
            container(Space::new(Length::Shrink, Length::Shrink))
        };

        container(column![graph_raw, graph_summary].spacing(10))
            .center_x()
            .width(iced::Length::Fill)
            .padding(10)
            .into()
    }

    fn draw_buttons(&self) -> iced::Element<Message> {
        let record = container(match self.record_file {
            Some(_) => button("Stop recording").on_press(Message::RecordStop),
            None => button("Record").on_press(Message::RecordStart),
        })
        .padding(10);
        let clear = container(button("Clear").on_press(Message::Clear)).padding(10);
        let toggle_graph =
            container(button("Toggle graph").on_press(Message::GraphToggle)).padding(10);
        let manual_trigger =
            container(button("Manual Trigger").on_press(Message::ManualTrigger)).padding(10);
        container(row![record, clear, toggle_graph, manual_trigger])
            .center_x()
            .width(iced::Length::Fill)
            .padding(10)
            .into()
    }

    fn draw_rate_selection(&self) -> iced::Element<Message> {
        let poll_rate_text = text("Poll rate");
        let poll_rate_options: Container<'_, Message> = container(pick_list(
            &PollRate::ALL[..],
            Some(self.selected_pollrate),
            Message::PollRateChanged,
        ));
        container(
            row![poll_rate_text, poll_rate_options]
                .align_items(Alignment::Center)
                .spacing(20),
        )
        .center_x()
        .width(iced::Length::Fill)
        .padding(10)
        .into()
    }

    fn draw_mode_selection(&self) -> iced::Element<Message> {
        let report_mode_text = text("Report mode");
        let report_mode_options = row![
            radio(
                ReportMode::Raw.to_string(),
                ReportMode::Raw,
                Some(self.selected_reportmode),
                Message::ReportModeChanged
            ),
            radio(
                ReportMode::Summary.to_string(),
                ReportMode::Summary,
                Some(self.selected_reportmode),
                Message::ReportModeChanged
            ),
            radio(
                ReportMode::Combined.to_string(),
                ReportMode::Combined,
                Some(self.selected_reportmode),
                Message::ReportModeChanged
            )
        ]
        .spacing(20);
        container(
            row![report_mode_text, report_mode_options]
                .align_items(Alignment::Center)
                .spacing(20),
        )
        .center_x()
        .width(iced::Length::Fill)
        .padding(10)
        .into()
    }

    fn draw_action_selection(&self) -> iced::Element<Message> {
        let action_mode_text = text("Action mode");
        let action_mode_options = row![
            radio(
                ActionType::Mouse.to_string(),
                ActionType::Mouse,
                Some(self.selected_action_type),
                Message::ActionModeChanged
            ),
            radio(
                ActionType::Keyboard.to_string(),
                ActionType::Keyboard,
                Some(self.selected_action_type),
                Message::ActionModeChanged
            ),
        ]
        .spacing(20);
        container(
            row![
                action_mode_text,
                action_mode_options,
                match self.selected_action_type {
                    ActionType::Mouse => {
                        container(pick_list(
                            &MouseButton::ALL[..],
                            self.selected_action_key.mouse,
                            |key| Message::ActionKeyChanged(key as u8),
                        ))
                    }
                    ActionType::Keyboard => {
                        container(pick_list(
                            &KeyboardKey::ALL[..],
                            self.selected_action_key.keyboard,
                            |key| Message::ActionKeyChanged(key as u8),
                        ))
                    }
                },
            ]
            .align_items(Alignment::Center)
            .spacing(20),
        )
        .center_x()
        .width(iced::Length::Fill)
        .padding(10)
        .into()
    }

    fn threshold_selection(&self) -> iced::Element<Message> {
        let threshold_text = text(format!("Threshold: {}", self.threshold));
        let threshold_slider = slider(
            // i16::MIN..=i16::MAX,
            -4000..=4000,
            self.threshold,
            Message::ThresholdChanged,
        )
        .on_release(Message::ThresholdReleased)
        .step(10i16)
        .shift_step(1i16);
        container(
            row![threshold_text, threshold_slider]
                .align_items(Alignment::Center)
                .spacing(20),
        )
        .center_x()
        .width(iced::Length::Fill)
        .padding(10)
        .into()
    }

    fn get_port() -> Result<Box<dyn SerialPort>, serialport::Error> {
        let ports = serialport::available_ports()?;
        serialport::new(&ports.first().expect("No Serial Ports").port_name, 115_200)
            .timeout(Duration::from_secs(100_000))
            .open()
    }

    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    #[allow(clippy::unused_self)]
    // just for polling fakeldat
    pub fn subscription(&self) -> Subscription<Message> {
        // for raw it needs to be at least (pollrate/256)
        let hertz = self.forced_tick_rate.map_or_else(
            || {
                match self.selected_reportmode {
                    ReportMode::Raw | ReportMode::Combined => {
                        std::convert::Into::<u16>::into(self.selected_pollrate) / 200
                    }
                    ReportMode::Summary => 10,
                }
                .clamp(10, u16::MAX)
            },
            |forced_tick_rate| forced_tick_rate,
        );
        iced::time::every(Duration::from_micros(1_000_000 / u64::from(hertz)))
            .map(|_| Message::Tick)
    }

    fn push_data(&mut self, data: RawReport) {
        // 4 seconds of data
        let sample_count = std::convert::Into::<u16>::into(self.selected_pollrate) as usize * 4;
        match self.raw_data.len().cmp(&sample_count) {
            Ordering::Less => {}
            Ordering::Equal => _ = self.raw_data.pop_front(),
            Ordering::Greater => self.raw_data = vec![].into(),
        };
        self.raw_data.push_back(data);
    }
}

impl Chart<Message> for UI {
    type State = ();
    fn draw_chart<DB: DrawingBackend>(&self, state: &Self::State, root: DrawingArea<DB, Shift>) {
        _ = root.fill(&WHITE);
        let builder = ChartBuilder::on(&root);
        self.build_chart(state, builder);
    }
    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        let min = self
            .raw_data
            .iter()
            .fold(std::u64::MAX, |a, b| a.min(b.timestamp));
        let max = self
            .raw_data
            .iter()
            .fold(std::u64::MIN, |a, b| a.max(b.timestamp));
        let mut chart = builder
            .set_all_label_area_size(45)
            .top_x_label_area_size(20)
            .x_label_area_size(20)
            .build_cartesian_2d(min..max, 0u64..4096)
            .unwrap();
        
        let amount_to_skip = self.raw_data.len() / 4096 + 1;
        chart
            .draw_series(LineSeries::new(
                self.raw_data
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % amount_to_skip == 0)
                    .map(|(_, report)| (report.timestamp, report.brightness.into())),
                BLUE.stroke_width(2),
            ))
            .expect("Draw brightness line");
        chart
            .configure_mesh()
            .disable_mesh()
            .disable_x_axis()
            .y_label_formatter(&ToString::to_string)
            .draw()
            .expect("Draw mesh");
        chart
            .draw_series(self.trigger_timestamps.iter().filter_map(|trigger| {
                if *trigger > min {
                    Some(Rectangle::new([(*trigger, 4095), (*trigger, 0)], RED))
                } else {
                    None
                }
            }))
            .expect("Draw triggers");
        chart
            .draw_series(self.macro_timestamps.iter().filter_map(|timestamp| {
                if *timestamp > min {
                    Some(Rectangle::new([(*timestamp, 4095), (*timestamp, 0)], GREEN))
                } else {
                    None
                }
            }))
            .expect("Draw macros");
        // TODO: visualize the threshold
    }
}
