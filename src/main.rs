extern crate ansi_escapes;
extern crate ansi_term;

use std::process::Command;
use std::sync::mpsc;
use ansi_term::Color;
use std::time::Instant;
use std::clone::Clone;

enum ThreadMessage {
    Done(usize, std::process::Output),
    Error(usize, std::process::Output),
}

fn main() {
    let (cx, mut threads) = run_commands_from_stdin();

    {
        let mut first_write = true;
        loop {
            if let Ok(msg) = cx.recv_timeout(std::time::Duration::from_millis(500)) {
                match msg {
                    ThreadMessage::Done(i, output) => {
                        threads[i].is_running = false;
                        threads[i].set_output(output);
                    }
                    ThreadMessage::Error(i, output) => {
                        threads[i].had_error = true;
                        threads[i].is_running = false;
                        threads[i].set_output(output);
                    }
                }
            }

            if first_write {
                first_write = false;
            } else {
                print!("{}", ansi_escapes::CursorUp(threads.len() as u16 + 1));
            }
            println!("{}{}",
                     Color::White.bold().underline().paint("Running Parallel"),
                     ansi_escapes::EraseEndLine);
            for thread in (&threads).iter() {
                println!("{}{}", thread.colored_string(), ansi_escapes::EraseEndLine);
            }

            let mut all_done = true;
            for thread_state in &mut threads {
                if thread_state.had_error {
                    all_done = true;
                    break;
                }
                if thread_state.is_running {
                    all_done = false;
                    break;
                }
            }
            if all_done {
                println!("All done");
                break;
            }
        }
    }

    for thread_state in threads {
        if thread_state.output.len() > 0 {
            println!("Output for {}:\n{}\n\n", thread_state.command, thread_state.output);
        }
    }
}

struct ThreadState {
    is_running: bool,
    had_error: bool,
    start: Instant,
    command: String,
    output: String,
}

impl ThreadState {
    fn colored_string(&self) -> String {
        let start = if self.is_running {
            Color::Yellow.paint(format!("running {}s: ", self.start.elapsed().as_secs()))
                .to_string()
        } else if self.had_error {
            Color::Red.paint("error: ").to_string()
        } else {
            Color::Green.paint("done: ").to_string()
        };
        format!("{}{}", start, Color::White.paint(&self.command))
    }

    fn set_output(&mut self, output: std::process::Output) {
        let mut to = String::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stdout.len() > 0 {
            to.push_str(&format!("====STDOUT====\n{}", stdout));
        }
        if stderr.len() > 0 {
            to.push_str(&format!("====STDERR====\n{}", stderr));
        }
        self.output = to;
    }
}

fn run_commands_from_stdin() -> (mpsc::Receiver<ThreadMessage>, Vec<ThreadState>) {
    let mut threads = vec![];
    let (tx, cx) = mpsc::channel();
    let mut i = 0;
    loop {
        let mut command = String::new();
        match std::io::stdin().read_line(&mut command) {
            Ok(n) => {
                if n == 0 {
                    break;
                }
            },
            Err(e) => {
                println!("Error: {}", e);
                continue
            }
        };
        let command = String::from(command.trim());
        if command.len() == 0 {
            continue
        }
        println!("\"{}\"", command);
        let command_parts: Vec<String> = command.split(' ').map(|x| String::from(x)).collect();

        let tx1 = tx.clone();
        let command2 = command.clone();
        std::thread::spawn(move || {
            match Command::new(&command_parts[0])
                .args(command_parts[1..command_parts.len()].iter())
                .output() {
                Ok(output) => {
                    if !output.status.success() {
                        tx1.send(ThreadMessage::Error(i, output)).expect("Failed to send Error");
                    } else {
                        tx1.send(ThreadMessage::Done(i, output)).expect("Failed to send Done");
                    }
                }
                Err(e) => {
                    println!("Failed to start {}: {}", command2, e);
                    std::process::exit(1)
                }
            };
        });
        let ts = ThreadState {
            command: format!("{}", command),
            is_running: true,
            had_error: false,
            start: Instant::now(),
            output: String::from(""),

        };
        threads.push(ts);
        i += 1;
    }
    return (cx, threads);
}
