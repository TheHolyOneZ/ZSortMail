mod fake;

pub use fake::{AttachmentSpec, Failure, FakeGoogle, MessageSpec};

pub fn seed_sample(fake: &FakeGoogle, now_ms: i64) {
    const DAY: i64 = 86_400_000;
    const HOUR: i64 = 3_600_000;
    let me = "you@gmail.example";
    let base = |from: &str, subject: &str, text: &str, age: i64| MessageSpec {
        from: from.into(),
        to: vec![me.into()],
        subject: subject.into(),
        text: Some(text.into()),
        date_ms: now_ms - age,
        ..Default::default()
    };

    let first = fake.add_message(MessageSpec {
        thread: Some("t-launch".into()),
        ..base(
            "Maya Okafor <maya.okafor@northwind.example>",
            "Q3 launch checklist",
            "Hi, attaching the checklist for the Q3 launch. Can you look before Thursday's sync?",
            6 * DAY,
        )
    });
    let _ = first;
    fake.add_message(MessageSpec {
        thread: Some("t-launch".into()),
        from: me.into(),
        to: vec!["Maya Okafor <maya.okafor@northwind.example>".into()],
        labels: vec!["SENT".into()],
        in_reply_to: Some("<18c00001@fake.example>".into()),
        ..base(me, "Re: Q3 launch checklist", "Thanks Maya. I added three items under Support.", 6 * DAY - 2 * HOUR)
    });
    fake.add_message(MessageSpec {
        thread: Some("t-launch".into()),
        ..base(
            "Maya Okafor <maya.okafor@northwind.example>",
            "Re: Q3 launch checklist",
            "Thursday 10:00 works. I'll move the invite.\n\nOn Tue, you wrote:\n> Thanks Maya. I added three items under Support.",
            5 * DAY,
        )
    });
    fake.add_message(MessageSpec {
        html: Some(
            "<div style=\"font-family:Arial\"><h2>Your parcel is on its way</h2><p>Order <b>394-2211</b> has shipped.</p>\
             <img src=\"cid:logo@dhl\" alt=\"DHL\"><img src=\"https://tracker.example/pixel.gif\" width=1 height=1></div>"
                .into(),
        ),
        attachments: vec![
            AttachmentSpec { filename: "dhl-logo.png".into(), mime: "image/png".into(), bytes: vec![0x89, b'P', b'N', b'G'], content_id: Some("logo@dhl".into()) },
            AttachmentSpec {
                filename: "label-394-2211.pdf".into(),
                mime: "application/pdf".into(),
                bytes: b"%PDF-1.4\n% sample label\n%%EOF\n".to_vec(),
                content_id: None,
            },
        ],
        headers: vec![("Auto-Submitted".into(), "auto-generated".into())],
        ..base("DHL Parcel <noreply@parcel.dhl.example>", "Your parcel is on its way", "Order 394-2211 has shipped. Expected Tuesday.", 2 * DAY)
    });
    fake.add_message(MessageSpec {
        headers: vec![
            ("List-Id".into(), "<dispatch.weeklydispatch.example>".into()),
            ("List-Unsubscribe".into(), "<https://weeklydispatch.example/u/77>".into()),
            ("Precedence".into(), "bulk".into()),
        ],
        html: Some("<p>This week: <a href=\"https://example.org/boring?utm_source=dispatch\">the case for boring infrastructure</a>.</p>".into()),
        ..base("The Weekly Dispatch <hello@weeklydispatch.example>", "Issue 212: boring infrastructure", "This week: the case for boring infrastructure.", DAY)
    });
    fake.add_message(MessageSpec {
        headers: vec![],
        ..base(
            "=?UTF-8?B?QW3DqWxpZSBEdWJvaXM=?= <amelie@lepetitzinc.example>",
            "=?UTF-8?Q?R=C3=A9servation_pour_samedi?=",
            "Bonjour, votre réservation pour samedi est confirmée.",
            4 * DAY,
        )
    });
    fake.add_message(MessageSpec {
        text: None,
        text_bytes: Some(("iso-8859-1".into(), b"Un caf\xe9 jeudi ?".to_vec())),
        ..base("Jean Moreau <jean@mail.example>", "Caf\u{e9}", "", 3 * DAY)
    });
    fake.add_message(base(
        "Priya Raman <priya@parley.example>",
        "Intro from Dan",
        "Hi, Dan suggested I write. Do you have twenty minutes next week?",
        3 * HOUR,
    ));
}
