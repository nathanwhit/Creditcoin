use anyhow::Result;
use rand::rngs::ThreadRng;
use rand::thread_rng;
use rand::Rng;
use std::thread::Builder;
use std::thread::JoinHandle;

use crate::miner::Answer;
use crate::miner::Challenge;
use crate::miner::MainChannel;
use crate::primitives::H256;
use crate::utils::to_hex;
use crate::work::get_hasher;
use crate::work::is_valid_proof_of_work;
use crate::work::mkhash_into;
use crate::work::Hasher;

use super::WorkerChannel;

type Parent = MainChannel<Message, Answer>;
type Child = WorkerChannel<Answer, Message>;

#[derive(Debug, Clone)]
pub enum Message {
  Shutdown,
  Cancel,
  Challenge(Challenge),
}

pub struct Worker {
  channel: Parent,
  handles: Option<Vec<JoinHandle<()>>>,
}

impl Worker {
  pub fn new() -> Result<Self> {
    let mut parent = Parent::new();
    let num_cpus = num_cpus::get();
    let mut handles = Vec::with_capacity(num_cpus);

    for id in 0..num_cpus {
      let handle: JoinHandle<()> = Builder::new()
        .name("Miner".to_string())
        .spawn(Self::task(parent.add_worker(), id))
        .expect("Worker thread failed to spawn");
      handles.push(handle);
    }

    Ok(Self {
      channel: parent,
      handles: Some(handles),
    })
  }

  pub fn send(&mut self, challenge: Challenge) {
    self.channel.send(Message::Challenge(challenge));
  }

  pub fn recv(&self) -> Option<Answer> {
    self.channel.try_recv()
  }

  pub fn cancel(&mut self) {
    self.channel.send(Message::Cancel);
  }

  fn task(mut channel: Child, id: usize) -> impl FnOnce() {
    move || {
      let mut hasher: Hasher = get_hasher();
      let mut output: H256 = H256::new();
      let mut rng: ThreadRng = thread_rng();

      'outer: loop {
        debug!("Thread {}: Waiting for challenge", id);

        let mut challenge: Challenge = match channel.recv() {
          Message::Challenge(challenge) => challenge,
          Message::Shutdown => break 'outer,
          Message::Cancel => break 'outer,
        };

        debug!("Thread {}: Received challenge: {:?}", id, challenge);

        let mut nonce: u64 = rng.gen_range(0, u64::MAX);
        let mut score: u32 = 0;
        let mut found = false;

        'inner: loop {
          mkhash_into(
            &mut hasher,
            &mut output,
            &challenge.block_id,
            &challenge.peer_id,
            nonce,
          );

          if let Some(s) = is_valid_proof_of_work(&output, challenge.difficulty) {
            debug!(
              "Thread {}: Found nonce: {:?} -> {}",
              id,
              nonce,
              to_hex(&output)
            );
            score = s;
            found = true;
            break 'inner;
          }

          match channel.try_recv() {
            Some(Message::Challenge(update)) => {
              debug!("Thread {}: Received update: {:?}", id, update);

              challenge = update;
              nonce = rng.gen_range(0, u64::MAX);
            }
            Some(Message::Shutdown) => {
              break 'outer;
            }
            Some(Message::Cancel) => {
              debug!("Thread {}: Canceling!", id);
              break 'inner;
            }
            None => {
              nonce = nonce.wrapping_add(1);
            }
          }
        }
        if found {
          channel.send(Answer {
            challenge,
            nonce,
            score,
          });
          channel.recv();
        }
      }
    }
  }
}

impl Drop for Worker {
  fn drop(&mut self) {
    self.channel.send(Message::Shutdown);

    if let Some(handles) = self.handles.take() {
      for handle in handles {
        if let Err(error) = handle.join() {
          error!("Handle failed to join: {:?}", error);
        }
      }
    } else {
      error!("Handle is `None`");
    }
  }
}
