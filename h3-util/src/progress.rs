use std::thread;

use crossbeam::channel::Receiver;
use indicatif::{ProgressBar, ProgressStyle};

pub trait ProgressPosition {
    fn position(&self) -> u64;
}

impl ProgressPosition for usize {
    fn position(&self) -> u64 { *self as u64 }
}

pub struct Progress {
    progress_thread: Option<thread::JoinHandle<()>>
}

impl Progress {
    pub fn new(len: u64, recv: Receiver<impl ProgressPosition + Send + 'static>, title: &str) -> Self {
        let pbar = if atty::is(atty::Stream::Stdout) {
            let pb = ProgressBar::new(len);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:50.green/blue}] {pos}/{len} ({eta})")
                .progress_chars("#>-"));
            Some(pb)
        } else {
            None
        };
        let log_title = title.to_owned();
        let progress_thread = thread::spawn(move || {
            for progress_update in recv.iter() {
                if let Some(pb) = &pbar {
                    pb.set_position(progress_update.position() as u64);
                } else {
                    log::info!("{}: {}/{}", log_title, progress_update.position(), len);
                }
            }
            if let Some(pb) = &pbar {
                pb.set_message("done");
                pb.abandon();
            } else {
                log::info!("{}: done", log_title)
            }
        });

        Self { progress_thread: Some(progress_thread) }
    }

    pub fn finish(&mut self) {
        if let Some(pt) = self.progress_thread.take() {
            pt.join().unwrap();
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.finish()
    }
}

pub trait ApplyProgress<R, F> where F: FnOnce() -> R {
    fn apply(&mut self, _: F) -> R;
}

impl<R, F: FnOnce() -> R> ApplyProgress<R, F> for Progress {
    fn apply(&mut self, f: F) -> R {
        let r = f();
        self.finish();
        r
    }
}