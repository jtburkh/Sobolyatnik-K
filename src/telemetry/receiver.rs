use std::{
    io,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryIter},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::protocol::{DecodeError, PacketDecoder, TelemetryMessage};

const MAX_PACKET_SIZE: usize = 65_507;
const READ_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub enum ReceiverEvent {
    Message {
        received_at: Instant,
        source: SocketAddr,
        message: TelemetryMessage,
    },
    Malformed {
        received_at: Instant,
        source: SocketAddr,
        error: DecodeError,
    },
    SocketError(io::Error),
}

pub struct TelemetryReceiver {
    local_address: SocketAddr,
    events: Receiver<ReceiverEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl TelemetryReceiver {
    pub fn bind(address: impl ToSocketAddrs, decoder: Arc<dyn PacketDecoder>) -> io::Result<Self> {
        let socket = UdpSocket::bind(address)?;
        socket.set_read_timeout(Some(READ_TIMEOUT))?;
        let local_address = socket.local_addr()?;
        let (sender, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("rtv-telemetry-udp".to_owned())
            .spawn(move || {
                let mut buffer = vec![0_u8; MAX_PACKET_SIZE];
                while !worker_stop.load(Ordering::Relaxed) {
                    match socket.recv_from(&mut buffer) {
                        Ok((length, source)) => {
                            let received_at = Instant::now();
                            let event = match decoder.decode(&buffer[..length]) {
                                Ok(message) => ReceiverEvent::Message {
                                    received_at,
                                    source,
                                    message,
                                },
                                Err(error) => ReceiverEvent::Malformed {
                                    received_at,
                                    source,
                                    error,
                                },
                            };
                            if sender.send(event).is_err() {
                                break;
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                            ) => {}
                        Err(error) => {
                            if sender.send(ReceiverEvent::SocketError(error)).is_err() {
                                break;
                            }
                        }
                    }
                }
            })?;
        Ok(Self {
            local_address,
            events,
            stop,
            worker: Some(worker),
        })
    }

    pub fn local_address(&self) -> SocketAddr {
        self.local_address
    }

    pub fn try_iter(&self) -> TryIter<'_, ReceiverEvent> {
        self.events.try_iter()
    }
}

impl Drop for TelemetryReceiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{net::UdpSocket, sync::Arc, thread, time::Duration};

    use super::{ReceiverEvent, TelemetryReceiver};
    use crate::telemetry::protocol::{JsonDecoder, TelemetryMessage};

    #[test]
    fn receives_valid_and_malformed_packets_without_blocking_the_caller() {
        let receiver = TelemetryReceiver::bind("127.0.0.1:0", Arc::new(JsonDecoder)).unwrap();
        assert_eq!(receiver.try_iter().count(), 0);

        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender
            .send_to(
                br#"{"version":1,"type":"snapshot","timestamp_ms":10,"player":{"id":1,"position":[0,0,0],"heading":0},"ai":[]}"#,
                receiver.local_address(),
            )
            .unwrap();
        sender
            .send_to(b"not json", receiver.local_address())
            .unwrap();

        let mut events = Vec::new();
        for _ in 0..20 {
            events.extend(receiver.try_iter());
            if events.len() == 2 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(events.iter().any(|event| matches!(
            event,
            ReceiverEvent::Message {
                message: TelemetryMessage::Snapshot(_),
                ..
            }
        )));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ReceiverEvent::Malformed { .. }))
        );
    }
}
