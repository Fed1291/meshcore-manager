//! Reads a node's whole state into memory (`State`): what a backup writes to disk and what a
//! restore compares with the backup. Only reads: nothing here changes the node.
//!
//! All or nothing: a question the node does not answer is asked once more; still nothing, and the
//! read fails (the link, the node). What the firmware says it does not have is listed in
//! `State::missing`, never silently left out.

use anyhow::Result;

use super::{Data, Reply, State};
use crate::i18n::tf;
use crate::node::cli::{self, Cli};
use crate::node::companion::{Companion, Got, REPLY};
use crate::node::Kind;
use crate::proto as p;

/// A companion setting read with one command and one reply: (section, command, reply code)
const SINGLES: &[(&str, &[u8], u8)] = &[
    ("device", &[p::CMD_DEVICE_QUERY, p::APP_VERSION], p::RESP_DEVICE_INFO),
    ("tuning", &[p::CMD_GET_TUNING_PARAMS], p::RESP_TUNING_PARAMS),
    ("autoadd", &[p::CMD_GET_AUTOADD_CONFIG], p::RESP_AUTOADD_CONFIG),
    ("flood-scope", &[p::CMD_GET_DEFAULT_FLOOD_SCOPE], p::RESP_DEFAULT_FLOOD_SCOPE),
    ("custom-vars", &[p::CMD_GET_CUSTOM_VARS], p::RESP_CUSTOM_VARS),
];

/// The console commands of a node's regions (firmware 1.10+)
pub const REGION_COMMANDS: [&str; 3] = ["region", "region home", "region default"];

/// The access list's section, and the command its one reply is kept under
pub const ACL: &str = "acl";
pub const ACL_COMMAND: &str = "get acl";

/// The node stopped answering at `step`
pub fn silent(step: &str) -> anyhow::Error {
    anyhow::anyhow!(tf("backup-stopped-answering", &[("step", step.into())]))
}

/// One companion question, asked again once when nothing came. Some = the answer, None = the
/// firmware does not have it; silence twice = an error.
fn ask(node: &mut Companion, step: &str, frame: &[u8], wanted: u8) -> Result<Option<Vec<u8>>> {
    for _ in 0..2 {
        match node.ask(frame, wanted, REPLY)? {
            Got::Frame(reply) => return Ok(Some(reply)),
            Got::Refused => return Ok(None),
            Got::Silent => {}
        }
    }
    Err(silent(step))
}

/// Everything a companion holds. `private_key`: its key too (when the firmware lets it out).
/// `progress(step, done, total)` as it goes.
pub fn companion(node: &mut Companion, private_key: bool, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<State> {
    let total = SINGLES.len() + 4;
    let mut step = 0;
    let mut next = |name: &str| {
        progress(name, step, total);
        step += 1;
    };
    let mut state = State::default();
    let mut channel_slots = 0;
    fn single(state: &mut State, id: &str, secret: bool, answer: Option<Vec<u8>>) {
        match answer {
            Some(frame) => state.add(id, secret, Data::Frames(vec![frame])),
            // an older firmware without it, or one built without it (key export)
            None => state.missing.push(id.to_string()),
        }
    }

    next("self");
    let me = ask(node, "self", &p::app_start(), p::RESP_SELF_INFO)?;
    single(&mut state, "self", false, me);
    for (id, command, code) in SINGLES {
        next(id);
        let answer = ask(node, id, command, *code)?;
        if *code == p::RESP_DEVICE_INFO {
            channel_slots = answer.as_deref().and_then(p::DeviceInfo::parse).map_or(0, |d| d.max_channels);
        }
        single(&mut state, id, false, answer);
    }
    if private_key {
        next("private-key");
        let key = ask(node, "private-key", &[p::CMD_EXPORT_PRIVATE_KEY], p::RESP_PRIVATE_KEY)?;
        single(&mut state, "private-key", true, key);
    }

    // every slot the firmware has, empty ones included: one missing would be a hole
    next("channels");
    let mut channels = Vec::new();
    for slot in 0..channel_slots {
        // the reply must be for this slot: a late one for the slot before is skipped and the
        // slot asked again (once), so a channel never lands in another slot
        let mut got = None;
        for _ in 0..2 {
            match ask(node, "channels", &[p::CMD_GET_CHANNEL, slot], p::RESP_CHANNEL_INFO)? {
                Some(frame) if frame.get(1) == Some(&slot) => {
                    got = Some(frame);
                    break;
                }
                Some(_) => continue,
                None => break,
            }
        }
        match got {
            Some(frame) => channels.push(frame),
            None => return Err(silent("channels")),
        }
    }
    if channels.is_empty() {
        state.missing.push("channels".into());
    } else {
        state.add("channels", true, Data::Frames(channels));
    }

    // the list must hold as many contacts as its first frame announced (request_list)
    next("contacts");
    let mut contacts = None;
    for _ in 0..2 {
        contacts = node.request_list(&[p::CMD_GET_CONTACTS], p::RESP_CONTACT, p::RESP_END_OF_CONTACTS, REPLY)?;
        if contacts.is_some() {
            break;
        }
    }
    state.add("contacts", false, Data::Frames(contacts.ok_or_else(|| silent("contacts"))?));
    Ok(state)
}

/// One console command, asked again once when no reply line came; twice nothing = an error
pub fn run(node: &mut Cli, command: &str) -> Result<String> {
    for _ in 0..2 {
        if let Some(reply) = node.run(command, cli::REPLY)? {
            return Ok(reply);
        }
    }
    Err(silent(command))
}

/// Every setting of a repeater, room server or sensor (`private_key`: the secrets too) and its
/// regions
pub fn console(node: &mut Cli, kind: Kind, private_key: bool, progress: &mut dyn FnMut(&str, usize, usize)) -> Result<State> {
    let commands = cli::backup_commands(kind, private_key);
    // the settings, then the regions, then the access list
    let total = commands.len() + 2;
    let mut state = State::default();
    let (mut settings, mut secrets) = (Vec::new(), Vec::new());
    for (i, (command, secret)) in commands.into_iter().enumerate() {
        progress(&command, i, total);
        // the firmware answers every command, an unknown one with "??:" (kept as it is: a
        // restore skips it)
        let reply = run(node, &command)?;
        let list = if secret { &mut secrets } else { &mut settings };
        list.push(Reply { command, reply });
    }
    state.add("settings", false, Data::Replies(settings));
    if !secrets.is_empty() {
        state.add("secrets", true, Data::Replies(secrets));
    }
    // regions: the tree, home, default scope, as the node prints them (the tree is the format
    // `region load` takes back). A firmware without regions has none to keep.
    progress("regions", total - 2, total);
    let mut regions = Vec::new();
    for command in REGION_COMMANDS {
        let reply = run(node, command)?;
        if !cli::not_supported(&reply) {
            regions.push(Reply { command: command.into(), reply });
        }
    }
    if !regions.is_empty() {
        state.add("regions", false, Data::Replies(regions));
    }
    // the access list (who may log in as what), as the node printed it: one reply per line
    // ("PP KEY", node::acl), so the section counts its entries; an empty list is a section with
    // none. A firmware without `get acl`: named as not in the backup. Asked again once when
    // nothing came.
    progress(ACL, total - 1, total);
    let mut list = node.acl()?;
    if list.is_none() {
        list = node.acl()?;
    }
    match list {
        Some(text) => {
            let lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
            let replies = lines.map(|l| Reply { command: ACL_COMMAND.into(), reply: l.to_string() }).collect();
            state.add(ACL, false, Data::Replies(replies));
        }
        None => state.missing.push(ACL.into()),
    }
    Ok(state)
}
