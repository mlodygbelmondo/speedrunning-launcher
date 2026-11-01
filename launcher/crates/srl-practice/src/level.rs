//! Reading a world's `level.dat` (gzip-compressed NBT) for the three things the launcher needs:
//! the `DataVersion` (does the world fit 1.16.1?), the game version that last saved it, and the
//! `LevelName` the Singleplayer list shows, which is how the player finds the map.
//!
//! The steps (gunzip, a named root compound, then `Data`) follow Pandora Launcher's
//! `load_world_summary` (crates/backend/src/instance.rs, MIT). The reader itself is ours and only
//! keeps the fields we ask for; like Pandora's decoder it caps nesting depth, so a hostile file
//! can't overflow the stack.
use std::io::Read;

use anyhow::{Context, Result, bail, ensure};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelInfo {
    /// `Data.DataVersion`. Missing in worlds older than 1.9.
    pub data_version: Option<i32>,
    /// `Data.Version.Name`, e.g. `1.16.1`.
    pub version_name: Option<String>,
    /// `Data.LevelName`, with the author's `§` colour codes left in.
    pub level_name: Option<String>,
}

impl LevelInfo {
    /// The name the Singleplayer list shows, without colour codes: what the UI tells the player
    /// to look for.
    pub fn display_name(&self) -> Option<String> {
        self.level_name.as_deref().map(strip_formatting)
    }
}

/// Removes Minecraft's `§x` formatting codes.
pub fn strip_formatting(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// Reads a gzip-compressed `level.dat`.
pub fn read_level_dat(gz: &[u8]) -> Result<LevelInfo> {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(gz)
        // A real level.dat is a few KB; refuse a gzip bomb.
        .take(64 << 20)
        .read_to_end(&mut raw)
        .context("level.dat is not gzip")?;
    parse(&raw)
}

pub fn read_level_dat_file(path: &std::path::Path) -> Result<LevelInfo> {
    let gz = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    read_level_dat(&gz).with_context(|| path.display().to_string())
}

const MAX_DEPTH: usize = 512;

/// Where in the tree the reader is, for picking the fields.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Spot {
    Root,
    Data,
    Version,
    Other,
}

struct Reader<'a> {
    buf: &'a [u8],
    info: LevelInfo,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        ensure!(self.buf.len() >= n, "truncated NBT");
        let (head, rest) = self.buf.split_at(n);
        self.buf = rest;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into()?))
    }
    fn string(&mut self) -> Result<String> {
        let n = u16::from_be_bytes(self.take(2)?.try_into()?) as usize;
        // NBT strings are Java's modified UTF-8; lossy is fine for names.
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn array(&mut self, width: usize) -> Result<()> {
        let n = self.i32()?;
        ensure!(n >= 0, "negative array length");
        self.take(n as usize * width)?;
        Ok(())
    }

    /// Reads one payload of `tag` that sits at `spot` under `name`.
    fn payload(&mut self, tag: u8, spot: Spot, name: &str, depth: usize) -> Result<()> {
        ensure!(depth <= MAX_DEPTH, "NBT nested too deep");
        match tag {
            1 => drop(self.take(1)?),
            2 => drop(self.take(2)?),
            3 => {
                let v = self.i32()?;
                if spot == Spot::Data && name == "DataVersion" {
                    self.info.data_version = Some(v);
                }
            }
            4 | 6 => drop(self.take(8)?),
            5 => drop(self.take(4)?),
            7 => self.array(1)?,
            8 => {
                let s = self.string()?;
                match (spot, name) {
                    (Spot::Data, "LevelName") => self.info.level_name = Some(s),
                    (Spot::Version, "Name") => self.info.version_name = Some(s),
                    _ => {}
                }
            }
            9 => {
                let inner = self.u8()?;
                let n = self.i32()?;
                ensure!(n >= 0, "negative list length");
                for _ in 0..n {
                    self.payload(inner, Spot::Other, "", depth + 1)?;
                }
            }
            10 => {
                let here = match (spot, name) {
                    (Spot::Root, "Data") => Spot::Data,
                    (Spot::Data, "Version") => Spot::Version,
                    _ => Spot::Other,
                };
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    let child = self.string()?;
                    self.payload(t, here, &child, depth + 1)?;
                }
            }
            11 => self.array(4)?,
            12 => self.array(8)?,
            _ => bail!("unknown NBT tag {tag}"),
        }
        Ok(())
    }
}

/// Parses uncompressed NBT whose root is a named compound.
pub fn parse(raw: &[u8]) -> Result<LevelInfo> {
    let mut r = Reader {
        buf: raw,
        info: LevelInfo::default(),
    };
    ensure!(r.u8()? == 10, "level.dat root is not a compound");
    r.string()?;
    // The root's children are at Spot::Root: walk them as the root compound's payload with a
    // name that matches nothing.
    loop {
        let t = r.u8()?;
        if t == 0 {
            break;
        }
        let name = r.string()?;
        r.payload(t, Spot::Root, &name, 1)?;
    }
    Ok(r.info)
}

/// A minimal 1.16.1 `level.dat` (gzip NBT: `Data.DataVersion`, `Data.LevelName`,
/// `Data.Version.Name`) for sample worlds: the launcher's screenshot data and other crates' tests.
pub fn sample_level_dat(name: &str) -> Vec<u8> {
    use std::io::Write;
    fn named(out: &mut Vec<u8>, tag: u8, name: &str) {
        out.push(tag);
        out.extend((name.len() as u16).to_be_bytes());
        out.extend(name.as_bytes());
    }
    fn string(out: &mut Vec<u8>, s: &str) {
        out.extend((s.len() as u16).to_be_bytes());
        out.extend(s.as_bytes());
    }
    let mut n = Vec::new();
    named(&mut n, 10, "");
    named(&mut n, 10, "Data");
    named(&mut n, 3, "DataVersion");
    n.extend(2567i32.to_be_bytes());
    named(&mut n, 8, "LevelName");
    string(&mut n, name);
    named(&mut n, 10, "Version");
    named(&mut n, 8, "Name");
    string(&mut n, "1.16.1");
    n.extend([0, 0, 0]);
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(&n).expect("writing to a Vec");
    e.finish().expect("writing to a Vec")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// A small NBT writer for tests: just enough to build level.dat files.
    pub struct Nbt(pub Vec<u8>);
    impl Nbt {
        pub fn root() -> Self {
            let mut n = Nbt(vec![10]);
            n.name("");
            n
        }
        fn name(&mut self, s: &str) {
            self.0.extend((s.len() as u16).to_be_bytes());
            self.0.extend(s.as_bytes());
        }
        pub fn compound(mut self, name: &str) -> Self {
            self.0.push(10);
            self.name(name);
            self
        }
        pub fn end(mut self) -> Self {
            self.0.push(0);
            self
        }
        pub fn int(mut self, name: &str, v: i32) -> Self {
            self.0.push(3);
            self.name(name);
            self.0.extend(v.to_be_bytes());
            self
        }
        pub fn string(mut self, name: &str, v: &str) -> Self {
            self.0.push(8);
            self.name(name);
            self.name(v);
            self
        }
        pub fn long(mut self, name: &str, v: i64) -> Self {
            self.0.push(4);
            self.name(name);
            self.0.extend(v.to_be_bytes());
            self
        }
        pub fn list_of_ints(mut self, name: &str, v: &[i32]) -> Self {
            self.0.push(9);
            self.name(name);
            self.0.push(3);
            self.0.extend((v.len() as i32).to_be_bytes());
            for x in v {
                self.0.extend(x.to_be_bytes());
            }
            self
        }
        pub fn gzip(self) -> Vec<u8> {
            let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            e.write_all(&self.0).unwrap();
            e.finish().unwrap()
        }
    }

    /// A level.dat like 1.16.1 writes, with decoys: the same names in other places.
    pub fn level_dat(data_version: i32, name: &str, last_played: i64) -> Vec<u8> {
        Nbt::root()
            .compound("Data")
            .int("version", 19133)
            .long("LastPlayed", last_played)
            .compound("Player")
            .int("DataVersion", 1)
            .string("LevelName", "decoy")
            .end()
            .list_of_ints("ServerBrands", &[1, 2])
            .compound("Version")
            .string("Name", "1.16.1")
            .int("Id", data_version)
            .end()
            .int("DataVersion", data_version)
            .string("LevelName", name)
            .end()
            .end()
            .gzip()
    }

    #[test]
    fn reads_the_fields() {
        let info = read_level_dat(&level_dat(2567, "Zero Practice §7v1.2.2 by Mescht", 7)).unwrap();
        assert_eq!(info.data_version, Some(2567));
        assert_eq!(info.version_name.as_deref(), Some("1.16.1"));
        assert_eq!(
            info.display_name().as_deref(),
            Some("Zero Practice v1.2.2 by Mescht")
        );
    }

    #[test]
    fn refuses_garbage() {
        assert!(read_level_dat(b"not gzip").is_err());
        assert!(read_level_dat(&Nbt(vec![10, 0]).gzip()).is_err());
        let mut deep = Nbt::root();
        for _ in 0..600 {
            deep = deep.compound("x");
        }
        assert!(read_level_dat(&deep.gzip()).is_err());
    }
}
