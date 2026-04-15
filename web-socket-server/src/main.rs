extern crate ws;

use chrono::prelude::*;
use lazy_static::*;
use std::cell::Cell;
use std::fs::*;
use std::io::{self, prelude::*, BufRead, SeekFrom};
use std::rc::Rc;
use std::sync::Mutex;
use timer;
use std::sync::mpsc::{self, channel};

use ws::{listen, CloseCode, Error, Handler, Handshake, Message, Result, Sender};

lazy_static! {
    static ref CHAT_HISTORY_FILE: Mutex<File> = Mutex::new({
        OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open("./resources/chat_history.txt")
            .unwrap()
    });
}

#[derive(Clone, Debug)]
struct Server {
    out: Sender,
    count: Rc<Cell<u32>>,
    timer_created: Rc<Cell<bool>>
}

pub struct Timer {
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
    out: Sender,
    timer: timer::Timer,
    last_known_day: DateTime<Local>,
}

impl Timer {
    fn new(out: Sender) -> Timer {
        let (tx, rx) = channel();
        Timer {
            tx,
            rx,
            out,
            timer: timer::Timer::new(),
            last_known_day: Local.ymd(1970, 1, 1).and_hms(0, 1, 1),
        }
    }

    fn update_date(&mut self, tx: mpsc::Sender<String>) -> timer::Guard {
        let now: DateTime<Local> = Local::now();
        let now_in_millis = now.timestamp_millis();
        let last_known_midnight_in_millis = self.last_known_day.timestamp_millis() - self.last_known_day.num_seconds_from_midnight() as i64 * 1000;
        let millis_from_last_midnight = now_in_millis - last_known_midnight_in_millis;
        
        /*
         * If 24h or more have passed since last midnight
         * the new date is broadcasted to everyone in the chat
         * the chat file is updated
         */
        if millis_from_last_midnight > (24 * 60 * 60 * 1000) {
            let mut file = &*CHAT_HISTORY_FILE.lock().unwrap();
            let message = format!("<div class = \"chat-date\"><strong>{}</strong></div> ", now.format("%d/%m/%Y"));
            if let Err(e) = writeln!(file, "{}", message) {
                println!("Cannot save chat history: {:?}", e);
            }
            self.out.broadcast(message).unwrap();
            self.last_known_day = now;
        }

        let next_midnight_in_millis = last_known_midnight_in_millis + 24 * 60 * 60 * 1000;
        let millis_till_next_midnight = next_midnight_in_millis - now_in_millis;
        
        self.timer.schedule_with_delay(chrono::Duration::milliseconds(millis_till_next_midnight), move || {
            tx.send(String::from("update_date")).unwrap();
        })
    }
}

impl Handler for Server {
    fn on_open(&mut self, _: Handshake) -> Result<()> {
        
        /*
         * Create a timer that updates the date when necessary 
         * only on the first connection ever 
         */
        if self.timer_created.get() == false {
            self.timer_created.set(true);
            
            let out = self.out.clone();
            std::thread::spawn(move || {
                let mut date_updater = Timer::new(out);
                let mut _guard = date_updater.update_date(date_updater.tx.clone()); //Keep the guard to execute the callback
                loop {
                    let message = date_updater.rx.recv().unwrap();
                    if message == "update_date" {
                        _guard = date_updater.update_date(date_updater.tx.clone()); //Keep the guard to execute the callback
                    } 
                }
            });
        }

        println!(
            "New connection, the number of live connections is {}",
            self.count.get() + 1
        );
        // We have a new connection, so we increment the connection counter
        Ok(self.count.set(self.count.get() + 1))
    }

    fn on_message(&mut self, msg: Message) -> Result<()> {
        if msg.as_text().unwrap() == "get::" {
            //First connection from client
            println!(
                "New connection, the number of live connections is {}",
                self.count.get()
            );
            let out = self.out.clone();
            std::thread::spawn(move || {
                let mut file = &*CHAT_HISTORY_FILE.lock().unwrap();
                file.seek(SeekFrom::Start(0)).unwrap();
                let lines = io::BufReader::new(file).lines();
                for line in lines {
                    match line {
                        Ok(message) => {
                            out.send(message).unwrap();
                        }
                        Err(err) => {
                            println!("Error while sending chat history: {:?}", err);
                        }
                    }
                }
            });
        } else {
            println!("message received: {}", msg);

            //Update the chat history file
            let out = self.out.clone();
            std::thread::spawn(move || {
                
                let now: DateTime<Local> = Local::now();
                let mut message = String::from("<p class = \"message\">");
                message.push_str(&msg.as_text().unwrap()[6..]); //Remove the "post::" part
                message.push_str("</p>");
                message.push_str(&format!("<p class = \"message-date\">{}</p> ", now.format("%T")));
                
                let mut file = CHAT_HISTORY_FILE.lock().unwrap();
                if let Err(e) = writeln!(file, "{}", message) {
                    println!("Cannot save chat history: {:?}", e);
                }

                // Broadcast the message back
                out.broadcast(message).unwrap();
            });
        }
        Ok(())
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        match code {
            CloseCode::Normal => println!("A client is done with the connection."),
            CloseCode::Away => println!("A client is leaving the site."),
            CloseCode::Abnormal => {
                println!("Closing handshake failed! Unable to obtain closing status from client.")
            }
            _ => println!("The client encountered an error: {}", reason),
        }

        // The connection is going down, decrement the count
        self.count.set(self.count.get() - 1);
    }

    fn on_error(&mut self, err: Error) {
        println!("The server encountered an error: {:?}", err);
    }
}

fn main() {
    let count = Rc::new(Cell::new(0));
    let timer_created = Rc::new(Cell::new(false)); 
    
    listen("localhost:8081", |out| Server {
        out: out,
        count: count.clone(),
        timer_created: timer_created.clone(),
    })
    .unwrap()
}
