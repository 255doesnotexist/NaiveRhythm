use clap::Parser;
use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use std::fmt::Debug;
use thiserror::Error;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct Args {
    #[clap(short, long)]
    input: String,
    #[clap(short, long)]
    output: String,
}

pub type Bmp = u32;
pub type Key = u32;

pub struct Input {
    pub bmp: Bmp,
    pub keys: Vec<Key>,
}

pub struct Output {
    pub bmp: Bmp,
    pub beat: Vec<u32>,
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("bad magic")]
    BadMagic,
    #[error("bad bmp")]
    BadBmp,
    #[error("bad key time")]
    BadKey,
}

#[derive(Error, Debug)]
pub enum OutputError {
    #[error("buffer error")]
    IOError(#[from] std::io::Error),
}

fn parse(s: &str) -> Result<Input, ParseError> {
    use ParseError::*;
    let mut keys = Vec::new();
    let mut tokens = s.split([' ', '\n']);
    // magic
    if "naive-rhythm" != tokens.next().ok_or(BadMagic)? {
        return Err(BadMagic);
    }
    // bmp
    if "bmp" != tokens.next().ok_or(BadBmp)? {
        return Err(BadBmp);
    }
    let bmp_str = tokens.next().ok_or(BadBmp)?;
    let bmp: Bmp = bmp_str.parse().map_err(|_| BadBmp)?;
    // keys
    for key_str in tokens {
        if key_str.is_empty() {
            continue;
        }
        let key: u32 = key_str.parse().map_err(|_| BadKey)?;
        keys.push(key);
    }
    // input
    Ok(Input { bmp, keys })
}

pub fn solve(input: Input) -> Output {
    let bmp = input.bmp;
    let beat_ms = 60_000 / bmp;
    let mut beat: Vec<u32> = input
        .keys
        .into_iter()
        .map(|key| {
            let ans_0 = key / beat_ms;
            let ans_1 = key / beat_ms + 1;
            if key - ans_0 * beat_ms <= ans_1 * beat_ms - key {
                ans_0
            } else {
                ans_1
            }
        })
        .collect();
    beat.sort_unstable();
    let beat = beat
        .into_iter()
        .filter({
            let mut last = None;
            move |x| {
                let ret = last != Some(*x);
                last = Some(*x);
                ret
            }
        })
        .collect();
    Output { bmp, beat }
}

fn build(output: Output) -> Result<Box<[u8]>, OutputError> {
    use TrackEventKind::*;
    let ppq = 480;
    let bmp = output.bmp;
    let tempo = 60_000_000 / bmp;
    let format = Format::Parallel;
    let timing = Timing::Metrical(u15::new(ppq));
    let header = Header::new(format, timing);
    let track0 = vec![
        TrackEvent {
            delta: u28::new(0),
            kind: Meta(MetaMessage::TrackName(&[])),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: Meta(MetaMessage::Tempo(u24::new(tempo))),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: Meta(MetaMessage::EndOfTrack),
        },
    ];
    let track1 = {
        let mut track = vec![];
        for i in 0..output.beat.len() {
            let on_delta = if i == 0 { output.beat[0] } else { 0 };
            track.push(TrackEvent {
                delta: u28::new(on_delta * 115200 / bmp),
                kind: Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOn {
                        key: u7::new(60),
                        vel: u7::new(127),
                    },
                },
            });
            let off_delta = if i == output.beat.len() - 1 {
                1
            } else {
                output.beat[i + 1] - output.beat[i]
            };
            track.push(TrackEvent {
                delta: u28::new(off_delta * 115200 / bmp),
                kind: Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOff {
                        key: u7::new(60),
                        vel: u7::new(0),
                    },
                },
            });
        }
        track.push(TrackEvent {
            delta: u28::new(0),
            kind: Meta(MetaMessage::EndOfTrack),
        });
        track
    };
    let mut smf = Smf::new(header);
    smf.tracks = vec![track0, track1];
    let mut binary = Vec::new();
    smf.write_std(&mut binary)?;
    Ok(binary.into_boxed_slice())
}

fn main() {
    let args = Args::parse();
    let input_str = std::fs::read_to_string(args.input).expect("failed to read the input file");
    let input = parse(&input_str).expect("failed to parse the input");
    let output = solve(input);
    let output_bin = build(output).expect("failed to build the output");
    std::fs::write(args.output, output_bin).expect("failed to write the output file");
}
