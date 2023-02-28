use rand::{seq::SliceRandom, Rng, thread_rng };
use std::io::{self, stdin, stdout, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::{Duration, Instant};
use thread::JoinHandle;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

enum GameResult {
    Won,
    Lose
}

struct Game {
    secret_number: u32,
    level: usize,
}

const COUNTDOWN_SECONDS: u64 = 10;

impl Game {
    fn new(level: usize) -> Result<Self, String> {
        if level < 3 || level > 9 {
            return Err("Invalid level. The level should be between 3 and 9.".to_string());
        }

        let mut rng = thread_rng();
        let mut digits: Vec<u32> = (1..=9).collect(); // Vector of digits from 0 to 9
        digits.shuffle(&mut rng); // Shuffle the digits randomly

        // Add 0 at a random position in the remaining digits
        let zero_index = rng.gen_range(0..=digits.len());
        digits.insert(zero_index, 0);

        let number_str: String = digits.iter().take(level).map(|&digit| digit.to_string()).collect();
        let number = number_str.parse::<u32>().unwrap();

        Ok(Game { secret_number: number, level })
    }

    fn play(&self) -> Result<GameResult, String> {
        let (input_tx, input_rx) = channel();

        let stop_flag = Arc::new(Mutex::new(false));
        let thread_stop_flag = stop_flag.clone();

        let input_thread = Game::start_input_thread(input_tx, thread_stop_flag);
        let mut entered_chars = Vec::with_capacity(self.level);

        let mut start_time = Instant::now();
        let mut stdout = stdout().into_raw_mode().unwrap();

        let mut attempts = 0;

        loop {
            let remaining_time = COUNTDOWN_SECONDS.saturating_sub(start_time.elapsed().as_secs());
            if remaining_time == 0 {
                stdout.flush().unwrap();

                *stop_flag.lock().unwrap() = true;
                return Ok(GameResult::Lose)
            }

            if let Ok(key) = input_rx.try_recv() {
                entered_chars.push(key);
            }


            write!(stdout, "\r[{:2}s] Insert {} characters {}", remaining_time, self.level - entered_chars.len(), entered_chars.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("-")).unwrap();
            stdout.flush().unwrap();

            if entered_chars.len() == self.level {
                let (hits, blows) = self.check_guess(&entered_chars);
                if hits == self.level {
                    write!(stdout, "\r\n = You won in {} attempts = \n", attempts).unwrap();
                    stdout.flush().unwrap();

                    *stop_flag.lock().unwrap() = true;
                    return Ok(GameResult::Won);
                } else {
                    attempts += 1;
                    write!(stdout, "\n\r✅ HIT: {}, ❓ BLOW: {}\n\n", hits, blows).unwrap();
                    stdout.flush().unwrap();

                    start_time = Instant::now();
                    entered_chars.clear();
                }
            }

            thread::sleep(Duration::from_millis(50));
        }

        let _ = input_thread.join().unwrap();
    }

    fn start_input_thread(input_tx: Sender<char>, thread_stop_flag: Arc<Mutex<bool>>) -> JoinHandle<()> {
        thread::spawn(move || {
            let stdin = stdin();

            for key in stdin.keys() {
                if *thread_stop_flag.lock().unwrap() {
                    break;
                }

                if let Ok(key) = key {
                    if let Some(c) = Game::key_to_char(key) {
                        if c.is_ascii_digit() {
                            let _ = input_tx.send(c);
                        }
                    }
                } else {
                    break;
                }
            }

        })
    }

    #[deprecated(since = "0.0.1", note = "adding a countdown, use if you want to have something more static")]
    fn get_guess(&self) -> Result<u64, String> {
        loop {
            print!("Enter a {}-digit number with no repeated digits: ", self.level);
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;

            let guess: String = input.trim().parse().map_err(|_| "Invalid input.".to_string())?;
            let guess_len = guess.len();
            if guess_len != self.level {
                println!("The guess should have {} digits.", self.level);
                continue;
            }

            let guess_digits: Vec<u8> = guess.to_string().chars().map(|c| c.to_digit(10).unwrap() as u8).collect();
            let has_repeated_digits = guess_digits.len() != guess_digits.iter().collect::<std::collections::HashSet<_>>().len();
            if has_repeated_digits {
                println!("The guess should have no repeated digits.");
                continue;
            }

            return Ok(guess.parse().unwrap());
        }
    }

    fn key_to_char(key: Key) -> Option<char> {
        match key {
            Key::Char(c) => Some(c),
            _ => None,
        }
    }

    fn check_guess(&self, guess: &Vec<char>) -> (usize, usize) {
        let secret_digits: Vec<u8> = self.secret_number
            .to_string()
            .chars()
            .map(|c| c.to_digit(10).unwrap() as u8)
            .collect();

        let guess_digits: Vec<u8> = guess
            .iter()
            .map(|c| c.to_digit(10).unwrap() as u8)
            .collect();

        let mut hits = 0;
        let mut blows = 0;
        let mut matched = [false; 10];

        for (i, &secret_digit) in secret_digits.iter().enumerate() {
            let guess_digit = guess_digits[i];
            if secret_digit == guess_digit {
                hits += 1;
                matched[secret_digit as usize] = true;
            }
        }

        for &guess_digit in &guess_digits {
            if !matched[guess_digit as usize] {
                if secret_digits.contains(&guess_digit) {
                    blows += 1;
                }
            }
        }

        (hits, blows)
    }
}

fn main() {
    let output = Command::new("clear").output().unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));

    println!(r#"
Try to guess the mistery number!
=================================

        Choose the level and start typing your guess, but be aware that you only have 10s.

        What are "hits" and "blows"?
        - hits means that you've correctly guessed a digit in the correct position
        - blows means that you've guessed a digit correctly but in the wrong position.

        Keep guessing until you get it right before time runs out!"#
    );

    print!("\n Choose a level between 3 and 10, or enter 'q' to quit: ");
    stdout().flush().unwrap();
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();

    if input.trim() == "q" {
        println!("Thanks for playing!");
        return;
    }

    let level = match input.trim().parse() {
        Ok(level) => level,
        Err(_) => {
            println!("Invalid input. Please enter a number between 3 and 10, or 'q' to quit.");
            return;
        }
    };

    match Game::new(level) {
        Ok(game) => {
            match game.play() {
                Ok(GameResult::Won) => {
                    println!("\n\r🎉 Congratulations! \r\n")

                }
                Ok(GameResult::Lose) => {
                    println!("\n\r You Lose 💣 \r");
                }
                Err(err) => println!("Error: {}", err),
            }
        },
        Err(err) => println!("Error: {}", err),
    }

}
