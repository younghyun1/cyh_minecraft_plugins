//! Persistent seeded state on one CPU worker, with bounded admission and cancellation.

use crate::{Error, PredictionRequest, PredictionResponse, generator};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use tokio::sync::oneshot;

const QUEUE_CAPACITY: usize = 8;

struct Job {
    request: PredictionRequest,
    reply: oneshot::Sender<Result<PredictionResponse, Error>>,
}

/// Clonable in-process generator handle. Seed material never leaves this process.
#[derive(Clone)]
pub struct Predictor {
    sender: SyncSender<Job>,
}

impl Predictor {
    /// Start a single bounded worker. Dropping all handles closes and drains its queue.
    pub fn new() -> Result<Self, Error> {
        // Pumpkin uses Rayon internally. A private single-thread pool prevents
        // per-core density-buffer pools from multiplying retained memory.
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .thread_name(|_| "minecraft-biomes-cpu".into())
            .stack_size(8 * 1024 * 1024)
            .build()
            .map_err(|_| Error::Unavailable)?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("minecraft-biomes".into())
            .spawn(move || pool.install(|| run(receiver)))
            .map_err(|_| Error::Unavailable)?;
        Ok(Self { sender })
    }

    /// Reject overflow immediately; dropping this future cancels queued or active work.
    pub async fn predict(&self, request: PredictionRequest) -> Result<PredictionResponse, Error> {
        request.validate()?;
        let (reply, response) = oneshot::channel();
        match self.sender.try_send(Job { request, reply }) {
            Ok(()) => response.await.map_err(|_| Error::Unavailable)?,
            Err(TrySendError::Full(_)) => Err(Error::Busy),
            Err(TrySendError::Disconnected(_)) => Err(Error::Unavailable),
        }
    }
}

fn next(receiver: &Receiver<Job>) -> Option<Job> {
    while let Ok(job) = receiver.recv() {
        if !job.reply.is_closed() {
            return Some(job);
        }
    }
    None
}

fn run(receiver: Receiver<Job>) {
    let Some(mut job) = next(&receiver) else {
        return;
    };
    loop {
        let seed = job.request.seed;
        let dimension = job.request.dimension;
        let large_biomes = job.request.large_biomes;
        let surface = job.request.y.is_none();
        let router = generator::router(seed, dimension, large_biomes, surface);
        // The sampler borrows its router; this nested loop keeps both alive
        // without leaked allocations or self-referential ownership.
        let mut sampler = generator::Sampler::new(&router, dimension, seed);
        loop {
            let result = generator::sample(&job.request, &mut sampler, || job.reply.is_closed());
            let _ = job.reply.send(result);
            let Some(next) = next(&receiver) else { return };
            job = next;
            if job.request.seed != seed
                || job.request.dimension != dimension
                || job.request.large_biomes != large_biomes
                || job.request.y.is_none() != surface
            {
                break;
            }
        }
    }
}

#[cfg(test)]
#[path = "predictor_tests.rs"]
mod tests;
