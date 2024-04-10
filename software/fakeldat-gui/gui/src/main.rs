use ui::UI;
mod ui;

fn main() -> iced::Result {
    let program = iced::program("FakeLDAT", UI::update, UI::view)
        .theme(UI::theme)
        .subscription(UI::subscription);
    program.run()
}
