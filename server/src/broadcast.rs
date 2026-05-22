use satiscore::network::messages::Paquet;
use tokio::sync::broadcast;

const BROADCAST_CAPACITY: usize = 512;

pub fn channel() -> (broadcast::Sender<Paquet>, broadcast::Receiver<Paquet>) {
    broadcast::channel(BROADCAST_CAPACITY)
}
