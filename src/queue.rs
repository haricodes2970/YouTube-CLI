use crate::decoder::Decoder;
use crate::renderer::VideoFrame;
use anyhow::{anyhow, Result};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};
use std::thread;
/// A decoded frame paired with its presentation timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedFrame {
    pub frame: VideoFrame,
    /// Presentation time in seconds relative to the start of the video.
    pub pts_secs: f64,
}

/// Bounded frame queue between the decoder thread and the UI loop.
///
/// Uses `sync_channel` so the queue is genuinely bounded: the decoder
/// thread blocks on `push()` when the queue is full, applying backpressure
/// and preventing unbounded memory growth.
pub struct FrameQueue {
    tx: SyncSender<QueuedFrame>,
    rx: Receiver<QueuedFrame>,
    /// Capacity of the internal channel.
    capacity: usize,
}

impl FrameQueue {
    /// Create a new bounded frame queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = sync_channel(capacity);
        Self { tx, rx, capacity }
    }

    /// Sender handle, given to the decoder thread.
    pub fn sender(&self) -> SyncSender<QueuedFrame> {
        self.tx.clone()
    }

    /// Try to pop a frame without blocking. Returns `None` if the queue is
    /// empty (decoder hasn't produced a frame yet, or EOF reached).
    pub fn try_pop(&self) -> Option<QueuedFrame> {
        match self.rx.try_recv() {
            Ok(frame) => Some(frame),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Returns `true` if the sender side has been dropped (decoder thread
    /// finished), meaning no more frames will ever be produced.
    pub fn is_closed(&self) -> bool {
        matches!(self.rx.try_recv(), Err(TryRecvError::Disconnected))
    }

    /// Push a frame into the queue (called by the decoder thread).
    ///
    /// Blocks if the queue is full, applying backpressure to the decoder.
    pub fn push(&self, frame: QueuedFrame) {
        let _ = self.tx.send(frame);
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Spawn a decoder thread that reads frames from `decoder` and pushes them
/// into `queue` with presentation timestamps derived from frame rate.
pub struct DecoderThread {
    handle: Option<thread::JoinHandle<Result<()>>>,
}

impl DecoderThread {
    /// Start the decoder loop in a background thread.
    ///
    /// `frame_rate` is used to assign presentation timestamps. Seeking is not
    /// supported while the thread is running; seeking restarts the decoder
    /// externally (see `App`).
    pub fn start(mut decoder: Decoder, sender: SyncSender<QueuedFrame>, frame_rate: f64) -> Self {
        let width = decoder.width();
        let height = decoder.height();
        let frame_size = decoder.frame_size();

        let handle = thread::spawn(move || -> Result<()> {
            let frame_duration = if frame_rate > 0.0 {
                1.0 / frame_rate
            } else {
                1.0 / 30.0 // fallback assumption
            };
            let mut pts = 0.0f64;

            loop {
                let raw = match decoder.next_frame()? {
                    Some(raw) => raw,
                    None => break, // EOF
                };

                // Validate frame buffer size before enqueuing.
                if raw.len() != frame_size {
                    // Incomplete/truncated frame: stop decoding.
                    break;
                }

                let frame = VideoFrame {
                    width,
                    height,
                    pixels: raw,
                    timestamp_ms: (pts * 1000.0) as u64,
                };

                sender.send(QueuedFrame {
                    frame,
                    pts_secs: pts,
                })?;

                pts += frame_duration;
            }

            Ok(())
        });

        Self {
            handle: Some(handle),
        }
    }

    /// Returns `true` if the underlying thread has finished executing.
    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_some_and(|h| h.is_finished())
    }

    /// Wait for the decoder thread to finish and propagate any error.
    pub fn join(mut self) -> Result<()> {
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|e| anyhow!("decoder thread panicked: {e:?}"))?
        } else {
            Ok(())
        }
    }
}

impl Drop for DecoderThread {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_try_pop_returns_none_when_empty() {
        let q = FrameQueue::new(4);
        assert_eq!(q.try_pop(), None);
    }

    #[test]
    fn queue_push_and_pop() {
        let q = FrameQueue::new(4);
        let frame = QueuedFrame {
            frame: VideoFrame {
                width: 2,
                height: 2,
                pixels: vec![0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255],
                timestamp_ms: 0,
            },
            pts_secs: 0.0,
        };
        q.push(frame);
        let popped = q.try_pop().unwrap();
        assert_eq!(popped.frame.width, 2);
        assert_eq!(popped.frame.height, 2);
        assert_eq!(popped.pts_secs, 0.0);
        // Queue is now empty again.
        assert!(q.try_pop().is_none());
    }

    #[test]
    fn queue_is_not_closed_when_sender_alive() {
        let q = FrameQueue::new(4);
        assert!(!q.is_closed());
        // Push and pop a frame (is_closed() is destructive, so check it
        // only after the queue is drained).
        let frame = QueuedFrame {
            frame: VideoFrame {
                width: 1,
                height: 1,
                pixels: vec![0, 0, 0],
                timestamp_ms: 0,
            },
            pts_secs: 0.0,
        };
        q.push(frame.clone());
        assert_eq!(q.try_pop(), Some(frame));
        // Queue empty, sender alive → not closed.
        assert!(!q.is_closed());
    }
}
