#[cfg(windows)]
mod windows_bridge {
    use axum::{
        extract::{
            ws::{Message, WebSocket, WebSocketUpgrade},
            State,
        },
        response::IntoResponse,
        routing::get,
        Router,
    };
    use libloading::Library;
    use serde::Serialize;
    use std::{
        error::Error, ffi::c_void, mem, net::SocketAddr, path::PathBuf, ptr, slice, thread,
        time::Duration,
    };
    use tokio::sync::watch;

    const LEAP_SUCCESS: u32 = 0;
    const LEAP_TIMEOUT: u32 = 0xE201_0004;
    const EVENT_CONNECTION: i32 = 1;
    const EVENT_CONNECTION_LOST: i32 = 2;
    const EVENT_DEVICE: i32 = 3;
    const EVENT_DEVICE_LOST: i32 = 0x104;
    const EVENT_TRACKING: i32 = 0x100;
    const DEFAULT_BRIDGE_ADDR: &str = "127.0.0.1:6437";
    const DEFAULT_LEAPC_PATH: &str = r"C:\Program Files\Ultraleap\LeapSDK\lib\x64\LeapC.dll";

    type LeapConnection = *mut c_void;
    type LeapCreateConnection = unsafe extern "C" fn(*const c_void, *mut LeapConnection) -> u32;
    type LeapOpenConnection = unsafe extern "C" fn(LeapConnection) -> u32;
    type LeapPollConnection =
        unsafe extern "C" fn(LeapConnection, u32, *mut LeapConnectionMessage) -> u32;
    type LeapCloseConnection = unsafe extern "C" fn(LeapConnection);
    type LeapDestroyConnection = unsafe extern "C" fn(LeapConnection);

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapVector {
        x: f32,
        y: f32,
        z: f32,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapQuaternion {
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapBone {
        prev_joint: LeapVector,
        next_joint: LeapVector,
        width: f32,
        rotation: LeapQuaternion,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapDigit {
        finger_id: i32,
        bones: [LeapBone; 4],
        is_extended: u32,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapPalm {
        position: LeapVector,
        stabilized_position: LeapVector,
        velocity: LeapVector,
        normal: LeapVector,
        width: f32,
        direction: LeapVector,
        orientation: LeapQuaternion,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapHand {
        id: u32,
        flags: u32,
        hand_type: i32,
        confidence: f32,
        visible_time: u64,
        pinch_distance: f32,
        grab_angle: f32,
        pinch_strength: f32,
        grab_strength: f32,
        palm: LeapPalm,
        digits: [LeapDigit; 5],
        arm: LeapBone,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapFrameHeader {
        reserved: *mut c_void,
        frame_id: i64,
        timestamp: i64,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapTrackingEvent {
        info: LeapFrameHeader,
        tracking_frame_id: i64,
        hand_count: u32,
        hands: *const LeapHand,
        framerate: f32,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    struct LeapConnectionMessage {
        size: u32,
        event_type: i32,
        event: *const c_void,
        device_id: u32,
    }

    #[derive(Clone)]
    struct BridgeState {
        latest: watch::Receiver<String>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TrackingFrame {
        #[serde(rename = "type")]
        message_type: &'static str,
        frame_id: i64,
        timestamp: i64,
        framerate: f32,
        hands: Vec<TrackedHand>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TrackedHand {
        id: u32,
        chirality: &'static str,
        flags: u32,
        confidence: f32,
        pinch_distance: f32,
        pinch_strength: f32,
        grab_strength: f32,
        palm: TrackedPalm,
        digits: Vec<TrackedDigit>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TrackedPalm {
        position: [f32; 3],
        stabilized_position: [f32; 3],
        velocity: [f32; 3],
        normal: [f32; 3],
        direction: [f32; 3],
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TrackedDigit {
        extended: bool,
        tip: [f32; 3],
    }

    #[derive(Serialize)]
    struct StatusMessage<'a> {
        #[serde(rename = "type")]
        message_type: &'static str,
        state: &'a str,
        message: &'a str,
    }

    pub async fn run() -> Result<(), Box<dyn Error>> {
        let initial = status_json("starting", "Loading the Hyperion LeapC runtime");
        let (sender, receiver) = watch::channel(initial);
        thread::Builder::new()
            .name("hyperion-leapc-poll".to_owned())
            .spawn(move || {
                if let Err(error) = poll_hyperion(sender.clone()) {
                    let message = format!("Hyperion bridge stopped: {error}");
                    eprintln!("{message}");
                    let _ = sender.send(status_json("error", &message));
                }
            })?;

        let address: SocketAddr = std::env::var("HYPERION_BRIDGE_ADDR")
            .unwrap_or_else(|_| DEFAULT_BRIDGE_ADDR.to_owned())
            .parse()?;
        let app = Router::new()
            .route("/health", get(health))
            .route("/hands", get(hands_socket))
            .with_state(BridgeState { latest: receiver });
        let listener = tokio::net::TcpListener::bind(address).await?;
        println!("MeshGraph Hyperion bridge listening on ws://{address}/hands");
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn health(State(state): State<BridgeState>) -> String {
        state.latest.borrow().clone()
    }

    async fn hands_socket(
        upgrade: WebSocketUpgrade,
        State(state): State<BridgeState>,
    ) -> impl IntoResponse {
        upgrade.on_upgrade(move |socket| stream_hands(socket, state.latest))
    }

    async fn stream_hands(mut socket: WebSocket, mut latest: watch::Receiver<String>) {
        let initial = latest.borrow().clone();
        if socket.send(Message::Text(initial)).await.is_err() {
            return;
        }
        while latest.changed().await.is_ok() {
            let message = latest.borrow_and_update().clone();
            if socket.send(Message::Text(message)).await.is_err() {
                break;
            }
        }
    }

    fn poll_hyperion(sender: watch::Sender<String>) -> Result<(), Box<dyn Error>> {
        let dll_path = std::env::var_os("HYPERION_LEAPC_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_LEAPC_PATH));
        if !dll_path.is_file() {
            return Err(format!("LeapC.dll was not found at {}", dll_path.display()).into());
        }

        // LeapC owns the event memory until the next poll. Every frame is copied into
        // serializable Rust values before polling again, and the library stays loaded
        // for the lifetime of all function pointers.
        let library = unsafe { Library::new(&dll_path)? };
        let create: LeapCreateConnection = unsafe { *library.get(b"LeapCreateConnection\0")? };
        let open: LeapOpenConnection = unsafe { *library.get(b"LeapOpenConnection\0")? };
        let poll: LeapPollConnection = unsafe { *library.get(b"LeapPollConnection\0")? };
        let close: LeapCloseConnection = unsafe { *library.get(b"LeapCloseConnection\0")? };
        let destroy: LeapDestroyConnection = unsafe { *library.get(b"LeapDestroyConnection\0")? };

        let mut connection: LeapConnection = ptr::null_mut();
        let result = unsafe { create(ptr::null(), &mut connection) };
        if result != LEAP_SUCCESS || connection.is_null() {
            return Err(format!("LeapCreateConnection failed with 0x{result:08X}").into());
        }
        let result = unsafe { open(connection) };
        if result != LEAP_SUCCESS {
            unsafe { destroy(connection) };
            return Err(format!("LeapOpenConnection failed with 0x{result:08X}").into());
        }

        let _guard = ConnectionGuard {
            handle: connection,
            close,
            destroy,
        };
        let _ = sender.send(status_json(
            "connecting",
            "Waiting for the Hyperion tracking service",
        ));

        loop {
            let mut message = LeapConnectionMessage {
                size: mem::size_of::<LeapConnectionMessage>() as u32,
                event_type: 0,
                event: ptr::null(),
                device_id: 0,
            };
            let result = unsafe { poll(connection, 1000, &mut message) };
            if result == LEAP_TIMEOUT {
                continue;
            }
            if result != LEAP_SUCCESS {
                let detail = format!("LeapPollConnection failed with 0x{result:08X}");
                let _ = sender.send(status_json("error", &detail));
                thread::sleep(Duration::from_millis(250));
                continue;
            }

            match message.event_type {
                EVENT_CONNECTION => {
                    let _ = sender.send(status_json(
                        "connected",
                        "Connected to the Hyperion tracking service",
                    ));
                }
                EVENT_CONNECTION_LOST => {
                    let _ = sender.send(status_json(
                        "disconnected",
                        "Hyperion tracking service disconnected",
                    ));
                }
                EVENT_DEVICE => {
                    let _ = sender.send(status_json(
                        "device-connected",
                        "Ultraleap camera connected",
                    ));
                }
                EVENT_DEVICE_LOST => {
                    let _ = sender.send(status_json(
                        "device-disconnected",
                        "Ultraleap camera disconnected",
                    ));
                }
                EVENT_TRACKING if !message.event.is_null() => {
                    let event =
                        unsafe { ptr::read_unaligned(message.event.cast::<LeapTrackingEvent>()) };
                    let frame = copy_tracking_frame(event);
                    if let Ok(payload) = serde_json::to_string(&frame) {
                        let _ = sender.send(payload);
                    }
                }
                _ => {}
            }
        }
    }

    fn copy_tracking_frame(event: LeapTrackingEvent) -> TrackingFrame {
        let hands = if event.hands.is_null() {
            &[][..]
        } else {
            let count = usize::try_from(event.hand_count).unwrap_or(0).min(4);
            unsafe { slice::from_raw_parts(event.hands, count) }
        };
        TrackingFrame {
            message_type: "frame",
            frame_id: event.info.frame_id,
            timestamp: event.info.timestamp,
            framerate: event.framerate,
            hands: hands.iter().copied().map(copy_hand).collect(),
        }
    }

    fn copy_hand(hand: LeapHand) -> TrackedHand {
        let palm = hand.palm;
        let digits = hand.digits;
        TrackedHand {
            id: hand.id,
            chirality: if hand.hand_type == 0 { "left" } else { "right" },
            flags: hand.flags,
            confidence: hand.confidence,
            pinch_distance: hand.pinch_distance,
            pinch_strength: hand.pinch_strength,
            grab_strength: hand.grab_strength,
            palm: TrackedPalm {
                position: vector(palm.position),
                stabilized_position: vector(palm.stabilized_position),
                velocity: vector(palm.velocity),
                normal: vector(palm.normal),
                direction: vector(palm.direction),
            },
            digits: digits
                .iter()
                .map(|digit| TrackedDigit {
                    extended: digit.is_extended != 0,
                    tip: vector(digit.bones[3].next_joint),
                })
                .collect(),
        }
    }

    fn vector(value: LeapVector) -> [f32; 3] {
        [value.x, value.y, value.z]
    }

    fn status_json(state: &str, message: &str) -> String {
        serde_json::to_string(&StatusMessage {
            message_type: "status",
            state,
            message,
        })
        .unwrap_or_else(|_| {
            r#"{"type":"status","state":"error","message":"Status serialization failed"}"#
                .to_owned()
        })
    }

    struct ConnectionGuard {
        handle: LeapConnection,
        close: LeapCloseConnection,
        destroy: LeapDestroyConnection,
    }

    impl Drop for ConnectionGuard {
        fn drop(&mut self) {
            unsafe {
                (self.close)(self.handle);
                (self.destroy)(self.handle);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ffi_layout_matches_hyperion_6_2_headers() {
            assert_eq!(mem::size_of::<LeapVector>(), 12);
            assert_eq!(mem::size_of::<LeapBone>(), 44);
            assert_eq!(mem::size_of::<LeapDigit>(), 184);
            assert_eq!(mem::size_of::<LeapPalm>(), 80);
            assert_eq!(mem::size_of::<LeapHand>(), 1084);
            assert_eq!(mem::size_of::<LeapTrackingEvent>(), 48);
            assert_eq!(mem::size_of::<LeapConnectionMessage>(), 20);
        }
    }
}

#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_bridge::run().await
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The MeshGraph Hyperion bridge currently requires Windows and LeapC.dll");
}
