// Copyright 2015 MaidSafe.net limited
//
// This MaidSafe Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the MaidSafe Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0, found in the root
// directory of this project at LICENSE, COPYING and CONTRIBUTOR respectively and also
// available at: http://www.maidsafe.net/licenses
//
// Unless required by applicable law or agreed to in writing, the MaidSafe Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS
// OF ANY KIND, either express or implied.
//
// See the Licences for the specific language governing permissions and limitations relating to
// use of the MaidSafe Software.

use std::net::{TcpListener, TcpStream, SocketAddr, Shutdown};
use std::io::{BufReader, ErrorKind};
use std::io::Result as IoResult;
use std::io::Error as IoError;
use cbor::{Encoder, CborError, Decoder};
use std::thread::spawn;
use std::marker::PhantomData;
use rustc_serialize::{Decodable, Encodable};
use bchannel::channel;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

//pub use bchannel::Receiver;
pub type InTcpStream<T> = Receiver<T>;

pub type TcpReader<T> = Receiver<T>;
pub type TcpWriter<T> = OutTcpStream<T>;

pub struct OutTcpStream<T> {
    tcp_stream: TcpStream,
    _phantom: PhantomData<T>
}

impl <'a, T> OutTcpStream<T>
where T: Encodable {
    pub fn send(&mut self, m: &T) -> Result<(), CborError> {
        let mut e = Encoder::from_writer(&mut self.tcp_stream);
        e.encode(&[&m])
    }

    pub fn send_all<'b, I: Iterator<Item = &'b T>>(&mut self, mut i: I) ->
    Result<(), (&'b T, I, CborError)> {
        loop {
            match i.next() {
                None => return Ok(()),
                Some(x) => {
                    match self.send(x) {
                        Ok(()) => {},
                        Err(e) => return Err((x, i, e))
                    }
                }
            }
        }
    }

    pub fn close(self) {
        self.tcp_stream.shutdown(Shutdown::Write).ok();
    }
}

#[unsafe_destructor]
impl <T> Drop for OutTcpStream<T> {
    fn drop(&mut self) {
        self.tcp_stream.shutdown(Shutdown::Write).ok();
    }
}

/// Connect to a server and open a send-receive pair.  See `upgrade` for more details.
pub fn connect_tcp<'a, 'b, I, O>(addr: SocketAddr) ->
IoResult<(Receiver<I>, OutTcpStream<O>)>
where I: Send + Decodable + 'static, O: Encodable {
    Ok(try!(upgrade_tcp(try!(TcpStream::connect(&addr)))))
}

/// Starts listening for connections on this ip and port.
/// Returns:
/// * A receiver of Tcp stream objects.  It is recommended that you `upgrade` these.
/// * A TcpAcceptor.  This can be used to close the listener from outside of the listening thread.
pub fn listen() ->  IoResult<(Receiver<(TcpStream, SocketAddr)>, TcpListener)> {
    let live_address = (("0.0.0.0"), 5483);
    let any_address = (("0.0.0.0"), 0);
    let tcp_listener = match TcpListener::bind(live_address) {
        Ok(x) => x,
        Err(_) => TcpListener::bind(&any_address).unwrap()
    };
    //println!("Listening on {:?}", tcp_listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();

    let tcp_listener2 = try!(tcp_listener.try_clone());
    spawn(move || {
        loop {
            // if tx.is_closed() {       // FIXME (Prakash)
            //     break;
            // }
            match tcp_listener2.accept() {
                Ok(stream) => {
                    if tx.send(stream).is_err() {
                        break;
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::TimedOut => {
                    continue;
                }
                Err(e) => {
                    //let _  = tx.error(e);
                    break;
                }
            }
        }
    });
    Ok((rx, tcp_listener))
}


// Almost a straight copy of https://github.com/TyOverby/wire/blob/master/src/tcp.rs
/// Upgrades a TcpStream to a Sender-Receiver pair that you can use to send and
/// receive objects automatically.  If there is an error decoding or encoding
/// values, that respective part is shut down.
pub fn upgrade_tcp<'a, 'b, I, O>(stream: TcpStream) -> IoResult<(InTcpStream<I>, OutTcpStream<O>)>
where I: Send + Decodable + 'static, O: Encodable {
    let s1 = stream;
    let s2 = try!(s1.try_clone());
    Ok((upgrade_reader(s1), upgrade_writer(s2)))
}

fn upgrade_writer<'a, T>(stream: TcpStream) -> OutTcpStream<T>
where T: Encodable {
    OutTcpStream {
        tcp_stream: stream,
        _phantom: PhantomData
    }
}

fn upgrade_reader<'a, T>(stream: TcpStream) -> InTcpStream<T>
where T: Send + Decodable + 'static {
    let (in_snd, in_rec) = mpsc::channel();

    spawn(move || {
        let mut buffer = BufReader::new(stream);
        {
            let mut decoder = Decoder::from_reader(&mut buffer);
            loop {
                let data = match decoder.decode().next() {
                  Some(a) => a,
                  None => { break; }
                  };
                match data {
                    Ok(a) => {
                        // Try to send, and if we can't, then the channel is closed.
                        if in_snd.send(a).is_err() {
                            break;
                        }
                    },
                    // if we can't decode, close the stream with an error.
                    Err(e) => {
                        // let _ = in_snd.error(e);
                        break;
                    }
                }
            }
        }
        let s1 = buffer.into_inner();
        let _ = s1.shutdown(Shutdown::Read);
    });
    in_rec
}



#[cfg(test)]
mod test {
    use super::*;
    use std::thread;
    use std::net::{SocketAddr};
    use std::str::FromStr;

// #[test]
    fn test_small_stream() {
        let (listener, u32) = listen().unwrap();
        let (i, mut o) = connect_tcp(SocketAddr::from_str("127.0.0.1:5483").unwrap()).unwrap();

        for x in 0u64 .. 10u64 {
            if o.send(&x).is_err() { break; }
        }
        o.close();
        thread::spawn(move || {
            for (connection, u32) in listener.into_blocking_iter() {
                // Spawn a new thread for each connection that we get.
                thread::spawn(move || {
                    let (i, mut o) = upgrade_tcp(connection).unwrap();
                    let i = i.blocking_iter(); 
                    let vec : Vec<u64> = i.collect();
                    for &x in vec.iter() {
                        if o.send(&(x, x + 1)).is_err() { break; }
                    }
                });
            }
        });
        // Collect everything that we get back.
        let mut responses: Vec<(u64, u64)> = Vec::new();
        for a in i.into_blocking_iter() {
            responses.push(a);
        }
        println!("Responses: {:?}", responses);
        assert_eq!(10, responses.len());
    }

// #[test]
    // fn test_multiple_client_small_stream() {
    //     const MSG_COUNT: usize = 5;
    //     const CLIENT_COUNT: usize = 101;

    //     let (listener, u32) = listen().unwrap();
    //     let mut vector_senders = Vec::new();
    //     let mut vector_receiver = Vec::new();
    //     for _ in 0..CLIENT_COUNT {
    //         let (i, o) = connect_tcp(SocketAddr::from_str("127.0.0.1:5483").unwrap()).unwrap();
    //         let boxed_output: Box<OutTcpStream<u64>> = Box::new(o);
    //         vector_senders.push(boxed_output);
    //         let boxed_input: Box<InTcpStream<(u64, u64)>> = Box::new(i);
    //         vector_receiver.push(boxed_input);
    //     }

    //     //  send
    //     for mut v in &mut vector_senders {
    //         for x in 0u64 .. MSG_COUNT as u64 {
    //             if v.send(&x).is_err() { break; }
    //         }
    //     }

    //     //  close sender
    //     loop {
    //        let sender = match vector_senders.pop() {
    //             None => break, // empty
    //             Some(sender) => sender.close(),
    //         };
    //     }
        

    //     // listener
    //     thread::spawn(move || {
    //         for (connection, u32) in listener.into_blocking_iter() {
    //             // Spawn a new thread for each connection that we get.
    //             thread::spawn(move || {
    //                 let (i, mut o) = upgrade_tcp(connection).unwrap();
    //                 let i = i.blocking_iter(); 
    //                 let vec : Vec<u64> = i.collect();
    //                 for &x in vec.iter() {
    //                     if o.send(&(x, x + 1)).is_err() { break; }
    //                 }
    //             });
    //         }
    //     });


    //     // Collect everything that we get back.
    //     let mut responses: Vec<(u64, u64)> = Vec::new();

    //     loop {
    //        let receiver = match vector_receiver.pop() {
    //             None => break, // empty
    //             Some(receiver) =>
    //             {
    //                 for a in (*receiver).into_blocking_iter() {
    //                     responses.push(a);
    //                 }
    //             }
    //         };
    //     }

    //     println!("Responses: {:?}", responses);
    //     assert_eq!((CLIENT_COUNT * MSG_COUNT), responses.len());
    // }


// #[test]
// fn test_stream_large_data() {
//     // Has to be sent over several packets
//     const LEN: usize = 1024 * 1024;
//     let data: Vec<u8> = (0..LEN).map(|idx| idx as u8).collect();
//     assert_eq!(LEN, data.len());
//
//     let d = data.clone(\;
//     let server_addr = next_test_ip4();
//     let mut server = UtpStream::bind(server_addr);
//
//     thread::spawn(move || {
//         let mut client = iotry!(UtpStream::connect(server_addr));
//         iotry!(client.write(&d[..]));
//         iotry!(client.close());
//     });
//
//     let read = iotry!(server.read_to_end());
//     assert!(!read.is_empty());
//     assert_eq!(read.len(), data.len());
//     assert_eq!(read, data);
// }

}
