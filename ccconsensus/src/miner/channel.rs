use std::sync::mpsc;

use crossbeam_channel::unbounded;
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use crossbeam_channel::TryRecvError;

pub struct MainChannel<T, U> {
  tx: bus::Bus<T>,
  rx: Receiver<U>,
  this_tx: Sender<U>,
}

impl<T, U> MainChannel<T, U>
where
  T: Send + Sync + 'static,
  U: Send + Sync + 'static,
{
  pub fn new() -> MainChannel<T, U> {
    let tx_p = bus::Bus::new(25);
    let (tx_c, rx_c): (Sender<U>, Receiver<U>) = unbounded();

    MainChannel {
      tx: tx_p,
      rx: rx_c,
      this_tx: tx_c,
    }
  }

  pub fn add_worker(&mut self) -> WorkerChannel<U, T> {
    let rx = self.tx.add_rx();
    let tx = self.this_tx.clone();
    WorkerChannel { rx, tx }
  }

  pub fn send(&mut self, message: T) {
    self.tx.broadcast(message);
  }

  pub fn recv(&self) -> U {
    self.rx.recv().expect("WorkerChannel Disconnected (recv)")
  }

  pub fn try_recv(&self) -> Option<U> {
    match self.rx.try_recv() {
      Ok(message) => Some(message),
      Err(TryRecvError::Empty) => None,
      Err(TryRecvError::Disconnected) => {
        panic!("WorkerChannel Disconnected (try_recv)");
      }
    }
  }
}

pub struct WorkerChannel<T, U> {
  tx: Sender<T>,
  rx: bus::BusReader<U>,
}

impl<T, U: Clone> WorkerChannel<T, U>
where
  T: Send + Sync + 'static,
  U: Send + Sync + 'static,
{
  pub fn send(&self, message: T) {
    self
      .tx
      .send(message)
      .expect("MainChannel Disconnected (send)");
  }

  pub fn recv(&mut self) -> U {
    self.rx.recv().expect("MainChannel Disconnected (recv)")
  }

  pub fn try_recv(&mut self) -> Option<U> {
    match self.rx.try_recv() {
      Ok(message) => Some(message),
      Err(mpsc::TryRecvError::Empty) => None,
      Err(mpsc::TryRecvError::Disconnected) => {
        panic!("MainChannel Disconnected (try_recv)")
      }
    }
  }
}
