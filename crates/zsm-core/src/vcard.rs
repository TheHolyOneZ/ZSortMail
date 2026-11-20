use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub emails: Vec<String>,
    pub organization: Option<String>,
    pub title: Option<String>,
    pub note: Option<String>,
}

fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !out.is_empty() {
            if let Some(last) = out.last_mut() {
                last.push_str(&raw[1..]);
            }
        } else {
            out.push(raw.to_string());
        }
    }
    out
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out.trim().to_string()
}

pub fn parse_vcards(text: &str) -> Vec<Contact> {
    let mut contacts = Vec::new();
    let mut current: Option<Contact> = None;
    let mut structured_name = String::new();
    for line in unfold(text) {
        let Some((head, value)) = line.split_once(':') else { continue };
        let property = head.split(';').next().unwrap_or("").rsplit('.').next().unwrap_or("").to_ascii_uppercase();
        match property.as_str() {
            "BEGIN" if value.trim().eq_ignore_ascii_case("VCARD") => {
                current = Some(Contact::default());
                structured_name.clear();
            }
            "END" if value.trim().eq_ignore_ascii_case("VCARD") => {
                if let Some(mut c) = current.take() {
                    if c.name.is_empty() {
                        c.name = std::mem::take(&mut structured_name);
                    }
                    c.emails.retain(|e| e.contains('@'));
                    c.emails.dedup();
                    if !c.emails.is_empty() {
                        contacts.push(c);
                    }
                }
            }
            _ => {
                let Some(c) = current.as_mut() else { continue };
                match property.as_str() {
                    "FN" => c.name = unescape(value),
                    "N" => {
                        let parts: Vec<String> = value.split(';').map(unescape).collect();
                        let given = parts.get(1).cloned().unwrap_or_default();
                        let family = parts.first().cloned().unwrap_or_default();
                        structured_name = format!("{given} {family}").trim().to_string();
                    }
                    "EMAIL" => c.emails.push(unescape(value).to_lowercase()),
                    "ORG" => {
                        let org = value.split(';').next().map(unescape).unwrap_or_default();
                        if !org.is_empty() {
                            c.organization = Some(org);
                        }
                    }
                    "TITLE" => {
                        let t = unescape(value);
                        if !t.is_empty() {
                            c.title = Some(t);
                        }
                    }
                    "NOTE" => {
                        let n = unescape(value);
                        if !n.is_empty() {
                            c.note = Some(n);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    contacts
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Maya Okafor\r\nN:Okafor;Maya;;;\r\nitem1.EMAIL;TYPE=INTERNET;TYPE=WORK:Maya.Okafor@Northwind.example\r\nEMAIL;TYPE=HOME:maya.home@gmail.example\r\nORG:Northwind;Design\r\nTITLE:Design lead\r\nNOTE:Likes oat milk.\\nCall before 10\\, never after.\r\nEND:VCARD\r\nBEGIN:VCARD\r\nVERSION:4.0\r\nN:Berg;Hanna;;;\r\nEMAIL:hanna@clinic.example\r\nEND:VCARD\r\nBEGIN:VCARD\r\nFN:No Address\r\nTEL:+49 123\r\nEND:VCARD\r\n";

    #[test]
    fn contacts_are_read_with_their_addresses_and_details() {
        let cards = parse_vcards(FILE);
        assert_eq!(cards.len(), 2, "a card with no email is left out");
        let maya = &cards[0];
        assert_eq!(maya.name, "Maya Okafor");
        assert_eq!(maya.emails, ["maya.okafor@northwind.example", "maya.home@gmail.example"]);
        assert_eq!((maya.organization.as_deref(), maya.title.as_deref()), (Some("Northwind"), Some("Design lead")));
        assert_eq!(maya.note.as_deref(), Some("Likes oat milk.\nCall before 10, never after."));
        assert_eq!(cards[1].name, "Hanna Berg", "the structured name is used when there is no FN");
    }

    #[test]
    fn folded_lines_and_stray_text_are_handled() {
        let cards =
            parse_vcards("junk\nBEGIN:VCARD\nFN:A very long\n  name here\nEMAIL:a@b.example\nEND:VCARD\nEND:VCARD\n");
        assert_eq!(cards[0].name, "A very long name here");
        assert!(parse_vcards("").is_empty());
        assert!(parse_vcards("BEGIN:VCARD\nFN:Open\nEMAIL:x@y.example\n").is_empty(), "an unfinished card is ignored");
    }
}
