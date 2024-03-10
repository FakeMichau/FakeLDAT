use std::{collections::VecDeque, env, fs::File, io::BufRead};

static SAMPLES: usize = 150;

fn calc_threshold(values: &VecDeque<usize>) -> usize {
    let sum: usize = values.iter().sum();
    let threshold = sum / values.len();
    threshold + 150
}

fn append_const_size<T>(vec: &mut VecDeque<T>, value: T) {
    if vec.len() == SAMPLES {
        vec.pop_front();
    }
    vec.push_back(value);
}

fn main() {
    let mut last_button_state: u8 = 0;
    let mut button_high_time: Option<u64> = None;
    let mut brightness_low_time: Option<u64> = None;
    let mut brightness_avg: VecDeque<usize> = VecDeque::with_capacity(SAMPLES);

    let serial_port = env::args()
        .filter(|x| x.contains("/dev/tty"))
        .last()
        .unwrap_or_else(|| "/dev/ttyACM0".to_string());
    let serial_handler = File::open(serial_port).expect("Unable to read file");
    let mut line = String::new();
    let mut buf_reader = std::io::BufReader::new(serial_handler);

    loop {
        line.clear();
        match buf_reader.read_line(&mut line) {
            Ok(_bytes_read) => {
                let data: Vec<&str> = line.trim().split(',').collect();
                if data.len() == 3 {
                    if let (Ok(time), Ok(brightness), Ok(button_pressed)) = (
                        data[0].parse::<u64>(),
                        data[1].parse::<usize>(),
                        data[2].parse::<u8>(),
                    ) {
                        append_const_size(&mut brightness_avg, brightness);

                        if last_button_state != button_pressed {
                            if button_pressed == 1 {
                                button_high_time = Some(time);
                            };
                            last_button_state = button_pressed;
                        }

                        if brightness > calc_threshold(&brightness_avg)
                            && brightness_low_time.is_none()
                            && button_high_time.is_some()
                        {
                            brightness_low_time = Some(time);

                            println!(
                                "Delay: {} Threshold: {}",
                                brightness_low_time.unwrap_or_default()
                                    - button_high_time.unwrap_or_default(),
                                calc_threshold(&brightness_avg)
                            );
                            button_high_time = None;
                            brightness_low_time = None;
                        }
                    }
                } else {
                    let data: Vec<&str> = line.trim().split(' ').collect();
                    if data.len() == 2 {
                        println!("Internal Delay: {} Threshold: {}", data[0], data[1]);
                    }
                }
            }
            Err(e) => eprintln!("Error reading line: {e}"),
        }
    }
}
