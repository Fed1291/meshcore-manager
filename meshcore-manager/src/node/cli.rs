//! Talking to a repeater, room server or sensor over USB: its text console.
//! A command is a line ended by CR; the node echoes it and prints "  -> <reply>"
//! (firmware: examples/simple_repeater/main.cpp, src/helpers/CommonCLI.cpp). A `get` answers
//! "> <value>". A command with nothing to say prints no reply line at all.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{Answer, Fact, Kind};
use crate::i18n::t;
use crate::regions::{self, NodeRegions};
use crate::serial::Link;

/// Every console command (firmware docs/cli_commands.md), with what it is for
const COMMANDS: &str = include_str!("../../data/cli_commands.toml");

/// How long a node gets for one reply
pub const REPLY: Duration = Duration::from_millis(1500);

/// How long the access list gets in all (32 lines of 68 characters take well under a second)
const ACL_WAIT: Duration = Duration::from_millis(3000);

#[derive(Deserialize)]
struct Entry {
    usage: String,
    #[serde(default)]
    about: String,
    /// The kinds that have it ("repeater", "room"); empty = every kind
    #[serde(default)]
    only: Vec<String>,
    #[serde(default)]
    secret: bool,
    #[serde(default)]
    danger: bool,
    #[serde(default)]
    serial: bool,
}

impl Entry {
    fn for_kind(&self, kind: Kind) -> bool {
        let own = match kind {
            Kind::Repeater => "repeater",
            Kind::RoomServer => "room",
            Kind::Sensor => "sensor",
            _ => "",
        };
        self.only.is_empty() || self.only.iter().any(|k| k == own)
    }
}

fn list() -> Vec<Entry> {
    toml::from_str::<List>(COMMANDS).map(|l| l.commands).unwrap_or_default()
}

/// A setting a console node lets change with `set <name> <value>`
pub struct Settable {
    /// "tx", "advert.interval": also the name its `get` is shown under
    pub name: String,
    /// What the value looks like: "<1-22 dBm>" style, from the command list
    pub hint: String,
    /// It restarts the node, or changes how it transmits: said so before a commit
    pub danger: bool,
    /// What it does (the command list's own words, from the firmware's docs)
    pub about: String,
}

/// The admin password: changed with its own command, `password <new>` (CommonCLI.cpp), not `set`
pub const ADMIN_PASSWORD: &str = "password";

/// Every setting of this kind of node that can be changed (the ones only for a terminal by hand
/// left out)
pub fn settable(kind: Kind) -> Vec<Settable> {
    list()
        .into_iter()
        .filter(|c| c.for_kind(kind) && !c.serial)
        .filter_map(|c| {
            let (name, hint) = match c.usage.strip_prefix("set ") {
                Some(rest) => rest.split_once(' ')?,
                None => c.usage.split_once(' ').filter(|(command, _)| *command == ADMIN_PASSWORD)?,
            };
            Some(Settable { name: name.to_string(), hint: hint.to_string(), danger: c.danger, about: c.about })
        })
        .collect()
}

#[derive(Deserialize)]
struct List {
    commands: Vec<Entry>,
}

/// The commands that read a node's settings: every `get <setting>` of its kind that takes no value.
/// With each: true when its reply is a secret (password, private key).
pub fn backup_commands(kind: Kind, secrets: bool) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = list()
        .into_iter()
        .filter(|c| c.usage.starts_with("get ") && !c.usage.contains('<') && !c.usage.contains('['))
        .filter(|c| c.for_kind(kind))
        // printed straight to the console, not as a reply line: read apart (`Cli::acl`)
        .filter(|c| c.usage != "get acl")
        .filter(|c| secrets || !c.secret)
        .map(|c| (c.usage, c.secret))
        .collect();
    out.dedup();
    out
}

/// A reply that says the node has no such setting: "Error: unsupported", "??: bridge.enabled"
/// (CommonCLI.cpp answers an unknown command with "??: <command>")
pub fn not_supported(reply: &str) -> bool {
    let lower = reply.to_ascii_lowercase();
    lower.starts_with("??")
        || lower.starts_with("error")
        || lower.starts_with("err:")
        || lower.contains("unsupported")
        || lower.starts_with("unknown command")
}

/// What a console node says about itself
pub struct Hello {
    pub kind: Kind,
    pub name: String,
    pub board: String,
    pub version: String,
    pub public_key: String,
    pub radio: String,
}

pub struct Cli {
    link: Link,
}

impl Cli {
    pub fn new(link: Link) -> Cli {
        Cli { link }
    }

    /// Sends one command. None = no reply line came within `wait`.
    pub fn run(&mut self, command: &str, wait: Duration) -> Result<Option<String>> {
        // its first words only ("set name", "get radio"): a value can be a password or a name
        let words: Vec<&str> = command.split_whitespace().collect();
        let head = match words.as_slice() {
            ["get" | "set", name, ..] => format!("{} {name}", words[0]),
            [first, ..] => first.to_string(),
            [] => String::new(),
        };
        crate::debug_log!("DEBUG", "-> console: {head}");
        self.link.write(format!("{command}\r").as_bytes()).with_context(|| t("serial-lost"))?;
        let deadline = Instant::now() + wait;
        let mut seen: Vec<u8> = Vec::new();
        let mut buf = [0u8; 512];
        loop {
            let n = self.link.read(&mut buf).with_context(|| t("serial-lost"))?;
            seen.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&seen);
            // the reply line is there and the node went quiet: done
            if n == 0 && text.contains("-> ") {
                break;
            }
            if Instant::now() >= deadline {
                break;
            }
        }
        let text = String::from_utf8_lossy(&seen);
        // the size only: a reply can be a password
        crate::debug_log!("DEBUG", "<- console: {} bytes{}", seen.len(), if text.contains("-> ") { "" } else { ", no reply line" });
        Ok(text.split_once("-> ").map(|(_, reply)| {
            // a `get` answers "> value"; an empty value is a lone ">"
            let reply = reply.trim();
            reply.strip_prefix('>').unwrap_or(reply).trim().to_string()
        }))
    }

    /// `set <name> <value>` (`password <value>` for the admin password). The firmware answers
    /// "OK" (sometimes with more: "OK - reboot to apply"), "password now: <it>", or an error
    /// text. A secret's reply is never passed on: it can hold the password.
    pub fn set(&mut self, name: &str, value: &str, secret: bool) -> Result<Answer> {
        let command = if name == ADMIN_PASSWORD { format!("{name} {value}") } else { format!("set {name} {value}") };
        Ok(match self.run(&command, REPLY)? {
            None => Answer::Silent,
            Some(reply) if reply.to_ascii_lowercase().starts_with("ok") || reply.starts_with("password now") => {
                Answer::Done
            }
            Some(_) if secret => Answer::Refused(t("edit-refused-secret")),
            Some(reply) => Answer::Refused(reply),
        })
    }

    /// Reads every setting of the node, for its own view: each `get` of its kind, shown under the
    /// setting's own name ("tx", "advert.interval"), and its clock. Secrets are marked
    /// (`Fact::secret`): the view hides them until asked.
    /// A setting that has a `set` is marked editable. What this board or firmware does not have
    /// is left out (no reply, or an error reply); its key and role are already in the view.
    pub fn settings(&mut self, kind: Kind) -> Result<Vec<Fact>> {
        let settable = settable(kind);
        let mut facts = Vec::new();
        if let Some(clock) = self.run("clock", REPLY)? {
            facts.push(self.sync_clock(clock)?);
        }
        // secrets too (guest password, private key...): kept hidden on screen until asked for
        for (command, secret) in backup_commands(kind, true) {
            let name = command.trim_start_matches("get ").to_string();
            if name == "public.key" || name == "role" {
                continue;
            }
            let Some(reply) = self.run(&command, REPLY)? else { continue };
            if not_supported(&reply) {
                continue;
            }
            let fact = match settable.iter().find(|s| s.name == name) {
                Some(s) => Fact::setting(name.clone(), reply, &name, &s.hint, s.danger).with_about(s.about.clone()),
                None => Fact::new(name, reply),
            };
            facts.push(if secret { fact.secret() } else { fact });
        }
        // the admin password has no `get`: the firmware cannot tell it, only take a new one
        if let Some(s) = settable.iter().find(|s| s.name == ADMIN_PASSWORD) {
            let fact = Fact::setting(s.name.clone(), String::new(), &s.name, &s.hint, s.danger);
            facts.push(fact.with_about(s.about.clone()).secret());
        }
        Ok(facts)
    }

    /// The clock line of the view. A clock behind this computer's (a node without a clock
    /// battery restarts on 15 May 2024) is set with `time <seconds>`: the firmware takes only a
    /// later time ("clock sync" needs the sender's time, which the serial console has not).
    fn sync_clock(&mut self, clock: String) -> Result<Fact> {
        // "12:06 - 15/5/2024 UTC"
        let read = chrono::NaiveDateTime::parse_from_str(clock.trim_end_matches("UTC").trim(), "%H:%M - %d/%m/%Y");
        let now = chrono::Utc::now().timestamp();
        let behind = read.is_ok_and(|at| at.and_utc().timestamp() < now - super::CLOCK_SLACK);
        if behind {
            if let Some(reply) = self.run(&format!("time {now}"), REPLY)? {
                if reply.to_ascii_lowercase().starts_with("ok") {
                    let set = self.run("clock", REPLY)?.unwrap_or(reply);
                    return Ok(Fact::new(t("fact-clock"), super::clock_synced(&set, &clock)).updated());
                }
            }
        }
        Ok(Fact::new(t("fact-clock"), clock))
    }

    /// A console command whose reply is "OK...", "Err - ..." or a text ("home is now x", the
    /// region tree): an "Err" reply is a refusal, any other a done
    pub fn command(&mut self, command: &str) -> Result<Answer> {
        Ok(match self.run(command, REPLY)? {
            None => Answer::Silent,
            Some(reply) if reply.to_ascii_lowercase().starts_with("err") => Answer::Refused(reply),
            Some(_) => Answer::Done,
        })
    }

    /// The node's regions (firmware 1.10+): the tree (`region`), its home and its default scope
    /// (1.15+). None: this firmware has no regions.
    pub fn regions(&mut self) -> Result<Option<NodeRegions>> {
        let Some(tree) = self.run("region", REPLY)?.filter(|r| !not_supported(r)) else { return Ok(None) };
        let home = self.run("region home", REPLY)?.filter(|r| !not_supported(r));
        let default = self.run("region default", REPLY)?.filter(|r| !not_supported(r));
        Ok(Some(regions::from_node(&tree, home.as_deref(), default.as_deref())))
    }

    /// The access list as the node prints it (`get acl`, node::acl): the text after "ACL:".
    /// The firmware prints it straight to the console (no "-> " reply line), so the end is when
    /// the node goes quiet. None: this firmware has no `get acl` (it answered with a reply line,
    /// or nothing came).
    pub fn acl(&mut self) -> Result<Option<String>> {
        const MARK: &str = "ACL:";
        // the list is done when nothing more came this long after the last byte
        const QUIET: Duration = Duration::from_millis(300);
        crate::debug_log!("DEBUG", "-> console: get acl");
        self.link.write(b"get acl\r").with_context(|| t("serial-lost"))?;
        let deadline = Instant::now() + ACL_WAIT;
        let mut last = Instant::now();
        let mut seen: Vec<u8> = Vec::new();
        let mut buf = [0u8; 512];
        loop {
            let n = self.link.read(&mut buf).with_context(|| t("serial-lost"))?;
            if n > 0 {
                seen.extend_from_slice(&buf[..n]);
                last = Instant::now();
            }
            let text = String::from_utf8_lossy(&seen);
            let quiet = n == 0 && last.elapsed() >= QUIET;
            // the list came and the node went quiet, or an answer that is not the list
            if quiet && (text.contains(MARK) || text.contains("-> ")) {
                break;
            }
            if Instant::now() >= deadline {
                break;
            }
        }
        let text = String::from_utf8_lossy(&seen);
        // the size only: the keys of who may log in
        crate::debug_log!("DEBUG", "<- console: {} bytes{}", seen.len(), if text.contains(MARK) { "" } else { ", no list" });
        Ok(text.split_once(MARK).map(|(_, list)| list.trim().to_string()))
    }

    /// Asks who is there. None = no MeshCore console answered.
    pub fn hello(&mut self) -> Result<Option<Hello>> {
        // ends whatever half line is in the node's buffer (the companion frames we tried before)
        let _ = self.run("", Duration::from_millis(400))?;
        let Some(kind) = self.run("get role", REPLY)?.and_then(|role| Kind::from_role(&role)) else {
            return Ok(None);
        };
        let mut ask = |command: &str| -> Result<String> { Ok(self.run(command, REPLY)?.unwrap_or_default()) };
        Ok(Some(Hello {
            kind,
            name: ask("get name")?,
            board: ask("board")?,
            version: ask("ver")?,
            public_key: ask("get public.key")?,
            radio: ask("get radio")?,
        }))
    }
}
