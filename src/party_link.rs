use crate::party_rewards::Request;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, io::{self, Read, Write}, net::{SocketAddr, TcpListener, TcpStream}, time::{Duration, Instant}};

pub const VERSION: u32 = 17;
const MAX_FRAME: usize = 256 * 1024;
const MAX_QUEUE: usize = 1024 * 1024;
const HELLO_TIMEOUT: Duration = Duration::from_secs(2);
fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidData, "invalid party link message") }

pub struct Credential { pub player: u64, pub token: String }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMessage { Choice(Request), Ready { epoch: u64 }, Game(crate::party_game::Report), Fault(String) }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HostMessage {
    Game(crate::party_game::Order),
    Paused(String),
    Snapshot { epoch: u64, state: String },
    Enter { epoch: u64, node: usize },
    Victory { epoch: u64, node: usize },
    Receipt { sequence: u64 },
}
#[derive(Debug)]
pub enum HostEvent { Connected(u64), Disconnected(u64), Message { player: u64, message: ClientMessage } }
#[derive(Debug)]
pub enum ClientEvent { Connected(u64), Disconnected, Message(HostMessage), Mismatch(u32) }
#[derive(Serialize, Deserialize)]
enum Wire {
    Hello { version: u32, session: u64, token: String },
    Welcome { version: u32, session: u64, player: u64 },
    Client(ClientMessage), Host(HostMessage), Ping, Pong,
}
struct Socket {
    stream: TcpStream, input: Vec<u8>, output: VecDeque<Vec<u8>>, offset: usize,
    queued: usize, created: Instant, received: Instant, ping: Instant,
}
impl Socket {
    fn new(stream: TcpStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        let now = Instant::now();
        Ok(Self { stream, input: vec![], output: VecDeque::new(), offset: 0, queued: 0, created: now, received: now, ping: now })
    }
    fn send(&mut self, message: Wire) -> io::Result<()> {
        let data = serde_json::to_vec(&message).map_err(|_| invalid())?;
        if data.len() > MAX_FRAME || self.queued + data.len() + 4 > MAX_QUEUE { return Err(invalid()); }
        let mut frame = (data.len() as u32).to_be_bytes().to_vec();
        frame.extend(data);
        self.queued += frame.len();
        self.output.push_back(frame);
        Ok(())
    }
    fn poll(&mut self) -> io::Result<Vec<Wire>> {
        if self.received.elapsed() > Duration::from_secs(30) { return Err(io::ErrorKind::TimedOut.into()); }
        if self.ping.elapsed() > Duration::from_secs(5) { self.send(Wire::Ping)?; self.ping = Instant::now(); }
        self.flush()?;
        let mut messages = vec![];
        let mut buffer = [0; 8192];
        for _ in 0..64 {
            match self.stream.read(&mut buffer) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => { self.input.extend_from_slice(&buffer[..n]); self.received = Instant::now(); }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
            while self.input.len() >= 4 {
                let len = u32::from_be_bytes(self.input[..4].try_into().unwrap()) as usize;
                if len == 0 || len > MAX_FRAME { return Err(invalid()); }
                if self.input.len() < len + 4 { break; }
                let message: Wire = serde_json::from_slice(&self.input[4..len+4]).map_err(|_| invalid())?;
                self.input.drain(..len+4);
                match message {
                    Wire::Ping => self.send(Wire::Pong)?,
                    Wire::Pong => {},
                    message => messages.push(message),
                }
                if messages.len() > 1024 { return Err(invalid()); }
            }
        }
        Ok(messages)
    }
    // Bound each flush so a noisy connection cannot starve other players.
    fn flush(&mut self) -> io::Result<()> {
        for _ in 0..64 {
            let Some(frame) = self.output.front() else { break };
            match self.stream.write(&frame[self.offset..]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    self.offset += n; self.queued -= n;
                    if self.offset == frame.len() { self.output.pop_front(); self.offset = 0; }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
pub struct Host { listener: TcpListener, session: u64, credentials: Vec<Credential>, peers: Vec<(Socket, Option<u64>)> }
impl Host {
    pub fn bind(address: SocketAddr, session: u64, credentials: Vec<Credential>) -> io::Result<Self> {
        if session == 0 || credentials.is_empty() || credentials.len() > 8 { return Err(invalid()); }
        for (i,c) in credentials.iter().enumerate() {
            if c.player == 0 || c.token.len() < 32 || c.token.len() > 256 || credentials[..i].iter().any(|p| p.player == c.player || p.token == c.token) { return Err(invalid()); }
        }
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, session, credentials, peers: vec![] })
    }
    pub fn address(&self) -> io::Result<SocketAddr> { self.listener.local_addr() }
    pub fn send(&mut self, player: u64, message: HostMessage) -> io::Result<()> {
        self.peers.iter_mut().find(|(_,id)| *id == Some(player)).ok_or(io::ErrorKind::NotConnected)?.0.send(Wire::Host(message))
    }
    pub fn poll(&mut self) -> Vec<HostEvent> {
        let mut events = vec![];
        for _ in 0..16 {
            match self.listener.accept() {
                Ok((stream,_)) if self.peers.len() < 16 => { if let Ok(socket) = Socket::new(stream) { self.peers.push((socket,None)); } }
                Ok(_) => {},
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        let mut i = 0;
        while i < self.peers.len() {
            let mut valid = self.peers[i].1.is_some() || self.peers[i].0.created.elapsed() < HELLO_TIMEOUT;
            match self.peers[i].0.poll() {
                Err(_) => valid = false,
                Ok(messages) => for message in messages {
                    if !valid { break; }
                    match (self.peers[i].1,message) {
                        (None,Wire::Hello { version, session, token }) if version == VERSION && session == self.session => {
                            let id = self.credentials.iter().find(|c| c.token == token).map(|c| c.player);
                            if let Some(id) = id.filter(|id| !self.peers.iter().any(|(_,p)| *p == Some(*id))) {
                                valid = self.peers[i].0.send(Wire::Welcome { version: VERSION, session, player: id }).is_ok();
                                if valid { self.peers[i].1 = Some(id); events.push(HostEvent::Connected(id)); }
                            } else { valid = false; }
                        }
                        (None,Wire::Hello { version, session, .. }) if version != VERSION => {
                            let socket = &mut self.peers[i].0;
                            if socket.send(Wire::Welcome { version: VERSION, session, player: 0 }).is_ok() { let _ = socket.flush(); }
                            valid = false;
                        }
                        (Some(player),Wire::Client(message)) => events.push(HostEvent::Message { player, message }),
                        _ => valid = false,
                    }
                }
            }
            if valid { i += 1; } else {
                let (_,id) = self.peers.remove(i);
                if let Some(id) = id { events.push(HostEvent::Disconnected(id)); }
            }
        }
        events
    }
}
pub struct Client { socket: Option<Socket>, session: u64, player: Option<u64> }
impl Client {
    // Connection establishment belongs on a worker thread, never the game tick.
    pub fn connect(address: SocketAddr, session: u64, token: String) -> io::Result<Self> {
        if session == 0 || token.len() > 256 { return Err(invalid()); }
        let mut socket = Socket::new(TcpStream::connect_timeout(&address, Duration::from_secs(3))?)?;
        socket.send(Wire::Hello { version: VERSION, session, token })?;
        Ok(Self { socket: Some(socket), session, player: None })
    }
    pub fn send(&mut self, message: ClientMessage) -> io::Result<()> {
        if self.player.is_none() { return Err(io::ErrorKind::NotConnected.into()); }
        self.socket.as_mut().ok_or(io::ErrorKind::NotConnected)?.send(Wire::Client(message))
    }
    pub fn poll(&mut self) -> Vec<ClientEvent> {
        let Some(socket) = self.socket.as_mut() else { return vec![]; };
        let mut events = vec![];
        let mut valid = self.player.is_some() || socket.created.elapsed() < Duration::from_secs(5);
        match socket.poll() {
            Err(_) => valid = false,
            Ok(messages) => for message in messages {
                if !valid { break; }
                match (self.player,message) {
                    (None, Wire::Welcome { version, session, player }) if version == VERSION && session == self.session && player != 0 => { self.player = Some(player); events.push(ClientEvent::Connected(player)); }
                    (None, Wire::Welcome { version, .. }) if version != VERSION => { events.push(ClientEvent::Mismatch(version)); valid = false; }
                    (Some(_),Wire::Host(message)) => events.push(ClientEvent::Message(message)),
                    _ => valid = false,
                }
            }
        }
        if !valid { self.socket = None; self.player = None; events.push(ClientEvent::Disconnected); }
        events
    }
}
// Reports are snapshots: only the latest per key matters. Queued items keep order and are never coalesced.
pub struct Backlog<T> { latest: Vec<(u64, T)>, queue: VecDeque<T>, limit: usize }
impl<T> Backlog<T> {
    pub fn new(limit: usize) -> Self { Self { latest: vec![], queue: VecDeque::new(), limit } }
    pub fn len(&self) -> usize { self.latest.len() + self.queue.len() }
    pub fn latest(&mut self, key: u64, item: T) {
        if let Some(slot) = self.latest.iter_mut().find(|(k,_)| *k == key) { slot.1 = item; } else { self.latest.push((key, item)); }
    }
    pub fn push(&mut self, item: T) -> bool {
        if self.queue.len() >= self.limit { return false; }
        self.queue.push_back(item); true
    }
    pub fn flush(&mut self, mut send: impl FnMut(T) -> Result<(), T>) {
        while let Some(item) = self.queue.pop_front() {
            if let Err(item) = send(item) { self.queue.push_front(item); return; }
        }
        while !self.latest.is_empty() {
            let (key, item) = self.latest.remove(0);
            if let Err(item) = send(item) { self.latest.insert(0, (key, item)); return; }
        }
    }
}
