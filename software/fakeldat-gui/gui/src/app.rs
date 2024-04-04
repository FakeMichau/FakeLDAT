use std::{cmp::Ordering, collections::VecDeque, time::Duration};

use fakeldat_lib::{serialport, ActionMode, FakeLDAT, RawReport, ReportMode, SummaryReport};
use iced::{
    widget::{
        button, column, container, pick_list, radio, row, scrollable, slider, text, Container,
        Rule, Scrollable, Space,
    },
    Alignment, Length, Subscription, Theme,
};
use plotters::{
    coord::Shift,
    element::Rectangle,
    series::LineSeries,
    style::{BLUE, RED, WHITE},
};
use plotters_iced::{Chart, ChartBuilder, ChartWidget, DrawingArea, DrawingBackend};

struct UI {
    fakeldat: FakeLDAT,
    theme: Theme,
    selected_pollrate: Option<PollRate>,
    selected_reportmode: Option<ReportMode>,
    selected_actionmode: Option<ActionMode>,
    selected_actionkey: Option<u8>,
    threshold: i16,
    show_graph: bool,
    raw_data: VecDeque<RawReport>,    // data refactor?
    summary_data: Vec<SummaryReport>, // TODO: old data is not being removed
    trigger: Vec<u64>,                // TODO: old data is not being removed
    init_process: u8,
}

impl Default for UI {
    fn default() -> Self {
        let ports = serialport::available_ports().expect("No ports found!");
        let port = serialport::new(&ports.first().expect("No Serial Ports").port_name, 115_200)
            .timeout(Duration::from_secs(100_000))
            .open()
            .expect("Failed to open port");
        Self {
            fakeldat: FakeLDAT::create(port).expect("Couldn't create FakeLDAT"),
            theme: Theme::Dark,
            selected_pollrate: Some(PollRate::_2000),
            selected_reportmode: Some(ReportMode::Raw),
            selected_actionmode: Some(ActionMode::Mouse),
            selected_actionkey: Some(0),
            threshold: 150,
            show_graph: true,
            raw_data: VecDeque::new(),
            summary_data: Vec::new(),
            trigger: Vec::new(),
            init_process: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Record,
    Clear,
    GraphToggle,
    PollRateChanged(PollRate), // TODO: change to actually value from the device, can't call the device in view, data needs to be retrieved in update
    ReportModeChanged(ReportMode),
    ActionModeChanged(ActionMode),
    ActionKeyChanged(u8),
    ThresholdChanged(i16),
    ThresholdReleased,
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
    const ALL: [Self; 7] = [
        Self::_500,
        Self::_1000,
        Self::_2000,
        Self::_4000,
        Self::_8000,
        Self::_16000,
        Self::_32000,
    ];
}

impl std::fmt::Display for PollRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::_500 => 500,
                Self::_1000 => 1000,
                Self::_2000 => 2000,
                Self::_4000 => 4000,
                Self::_8000 => 8000,
                Self::_16000 => 16000,
                Self::_32000 => 32000,
            }
        )
    }
}

impl TryFrom<u16> for PollRate {
    type Error = ();

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            500 => Ok(Self::_500),
            1000 => Ok(Self::_1000),
            2000 => Ok(Self::_2000),
            4000 => Ok(Self::_4000),
            x if x > 7750 && x < 8250 => Ok(Self::_8000),
            x if x > 15500 && x < 16500 => Ok(Self::_16000),
            x if x > 31000 && x < 33000 => Ok(Self::_32000),
            _ => Err(()),
        }
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
        // return; Drawing the chart is slow at >1000Hz
        let min = self
            .raw_data
            .iter()
            .fold(std::u64::MAX, |a, b| a.min(b.timestamp));
        let max = self
            .raw_data
            .iter()
            .fold(std::u64::MIN, |a, b| a.max(b.timestamp));
        let mut chart = builder
            .build_cartesian_2d(min as f64..max as f64, 0.0..4096.0)
            .unwrap();
        let nice = self
            .raw_data
            .iter()
            .map(|report| (report.timestamp as f64, f64::from(report.brightness)));
        chart.draw_series(LineSeries::new(nice, &BLUE));
        chart.configure_mesh().disable_mesh().draw();
        chart.draw_series(self.trigger.iter().filter_map(|trigger| {
            if *trigger > min {
                Some(Rectangle::new(
                    [(*trigger as f64, 4095.0), (*trigger as f64, 0.0)],
                    RED,
                ))
            } else {
                None
            }
        }));
        // TODO: visualize the threshold
        //build your chart here, please refer to plotters for more details
    }
}

impl UI {
    #[allow(clippy::needless_pass_by_value)]
    fn update(&mut self, message: Message) {
        match message {
            Message::Tick => {
                self.fakeldat.poll_bulk_data();
                if let Some(reports) = self.fakeldat.take_report_buffer() {
                    for report in reports {
                        match report {
                            fakeldat_lib::Report::Raw(raw_report) => {
                                if let Some(last_record) = self.raw_data.back() {
                                    if !last_record.trigger && raw_report.trigger {
                                        self.trigger.push(raw_report.timestamp);
                                    }
                                }
                                self.push_data(raw_report);
                            }
                            fakeldat_lib::Report::Summary(summary_report) => {
                                self.summary_data.push(summary_report);
                            }
                            fakeldat_lib::Report::PollRate(pollrate) => {
                                self.selected_pollrate = pollrate.try_into().ok();
                            }
                            fakeldat_lib::Report::Action(action_mode, key) => {
                                self.selected_actionmode = Some(action_mode);
                                self.selected_actionkey = Some(key);
                            }
                            fakeldat_lib::Report::ReportMode(report_mode) => {
                                self.selected_reportmode = Some(report_mode);
                            }
                            fakeldat_lib::Report::Threshold(threshold) => {
                                self.threshold = threshold;
                            }
                        }
                    }
                }

                // HACK: call for current settings while avoiding the buffer being cleared at the begining
                if self.init_process <= 50 {
                    self.init_process += 1;
                }
                if self.init_process == 50 {
                    self.fakeldat.get_action();
                    self.fakeldat.get_poll_rate();
                    self.fakeldat.get_threshold();
                    self.fakeldat.get_report_mode();
                }
            }
            Message::Record => {}
            Message::Clear => {
                self.raw_data = vec![].into();
                self.summary_data = vec![];
            }
            Message::GraphToggle => self.show_graph = !self.show_graph,
            Message::PollRateChanged(pollrate) => {
                let pollrate = pollrate.to_string().parse::<u16>().unwrap_or(1000);
                self.fakeldat.set_poll_rate(pollrate);
            }
            Message::ReportModeChanged(report_mode) => self.fakeldat.set_report_mode(report_mode),
            Message::ActionModeChanged(action_mode) => {
                self.selected_actionmode = Some(action_mode);
                self.selected_actionkey = None;
            }
            Message::ActionKeyChanged(key) => {
                self.fakeldat
                    .set_action(self.selected_actionmode.unwrap(), key);
            }
            Message::ThresholdChanged(threshold) => self.threshold = threshold,
            Message::ThresholdReleased => self.fakeldat.set_threshold(self.threshold),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn view(&self) -> iced::Element<Message> {
        let graph_raw = if self.show_graph
            && (self.selected_reportmode.unwrap() == ReportMode::Raw
                || self.selected_reportmode.unwrap() == ReportMode::Combined)
        {
            container(
                ChartWidget::new(self)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
        } else if !self.show_graph {
            container(Space::new(Length::Fill, Length::Fill))
        } else { // When showing the other graph
            container(Space::new(Length::Shrink, Length::Shrink))
        };
        let graph_summary = if self.show_graph
            && (self.selected_reportmode.unwrap() == ReportMode::Summary
                || self.selected_reportmode.unwrap() == ReportMode::Combined)
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
        } else { // When showing the other graph
            container(Space::new(Length::Shrink, Length::Shrink))
        };

        let graph = container(column![graph_raw, graph_summary].spacing(10))
            .center_x()
            .width(iced::Length::Fill)
            .padding(10);

        let record = container(button("Record").on_press(Message::Record)).padding(10);
        let clear = container(button("Clear").on_press(Message::Clear)).padding(10);
        let toggle_graph =
            container(button("Toggle graph").on_press(Message::GraphToggle)).padding(10);
        let mid = container(row![record, clear, toggle_graph])
            .center_x()
            .width(iced::Length::Fill)
            .padding(10);

        let spacer = Rule::horizontal(1);

        let poll_rate_text = text("Poll rate");
        let poll_rate_options: Container<'_, Message> = container(pick_list(
            &PollRate::ALL[..],
            self.selected_pollrate,
            Message::PollRateChanged,
        ));
        let poll_rate = container(
            row![poll_rate_text, poll_rate_options]
                .align_items(Alignment::Center)
                .spacing(20),
        )
        .center_x()
        .width(iced::Length::Fill)
        .padding(10);

        let report_mode_text = text("Report mode");
        let report_mode_options = row![
            radio(
                ReportMode::Raw.to_string(),
                ReportMode::Raw,
                self.selected_reportmode,
                Message::ReportModeChanged
            ),
            radio(
                ReportMode::Summary.to_string(),
                ReportMode::Summary,
                self.selected_reportmode,
                Message::ReportModeChanged
            ),
            radio(
                ReportMode::Combined.to_string(),
                ReportMode::Combined,
                self.selected_reportmode,
                Message::ReportModeChanged
            )
        ]
        .spacing(20);
        let report_mode = container(
            row![report_mode_text, report_mode_options]
                .align_items(Alignment::Center)
                .spacing(20),
        )
        .center_x()
        .width(iced::Length::Fill)
        .padding(10);

        let action_mode_text = text("Action mode");
        let action_mode_options = row![
            radio(
                ActionMode::Mouse.to_string(),
                ActionMode::Mouse,
                self.selected_actionmode,
                Message::ActionModeChanged
            ),
            radio(
                ActionMode::Keyboard.to_string(),
                ActionMode::Keyboard,
                self.selected_actionmode,
                Message::ActionModeChanged
            ),
        ]
        .spacing(20);
        let action_mode = container(
            row![
                action_mode_text,
                action_mode_options,
                pick_list(
                    // TODO: convert them into chars and "LMB, RMB" respectively
                    match self.selected_actionmode.expect("Selected Action mode") {
                        ActionMode::Mouse => vec![0, 1, 2],
                        ActionMode::Keyboard => (97..=122).collect(),
                    },
                    self.selected_actionkey,
                    Message::ActionKeyChanged,
                )
            ]
            .align_items(Alignment::Center)
            .spacing(20),
        )
        .center_x()
        .width(iced::Length::Fill)
        .padding(10);

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
        let threshold = container(
            row![threshold_text, threshold_slider]
                .align_items(Alignment::Center)
                .spacing(20),
        )
        .center_x()
        .width(iced::Length::Fill)
        .padding(10);

        let main_stack = column![
            graph,
            mid,
            spacer,
            poll_rate,
            report_mode,
            action_mode,
            threshold,
        ];

        container(main_stack)
            .center_x()
            .center_y()
            .padding(20)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    #[allow(clippy::unused_self)]
    // just for polling fakeldat
    fn subscription(&self) -> Subscription<Message> {
        const HERTZ: u64 = 40;
        iced::time::every(Duration::from_micros(1_000_000 / HERTZ)).map(|_| Message::Tick)
    }

    fn push_data(&mut self, data: RawReport) {
        // 4 seconds of data
        let sample_count = self
            .selected_pollrate
            .unwrap()
            .to_string()
            .parse::<usize>()
            .unwrap_or_default()
            * 4;
        match self.raw_data.len().cmp(&sample_count) {
            Ordering::Less => {}
            Ordering::Equal => _ = self.raw_data.pop_front(),
            Ordering::Greater => self.raw_data = vec![].into(),
        };
        self.raw_data.push_back(data);
    }
}

pub fn run() -> iced::Result {
    let program = iced::program("FakeLDAT", UI::update, UI::view)
        .theme(UI::theme)
        .subscription(UI::subscription);
    program.run()
}
