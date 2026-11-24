use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};

use crate::clock::{DAY, HOUR, MINUTE};
use crate::error::Result;
use crate::ink::ink_for;
use crate::store::Store;
use crate::store::ingest::{Address, HeaderFacts, IncomingAttachment, IncomingMessage, IngestContext};

pub const DEMO_EMAIL: &str = "you@zsortmail.demo";

#[derive(Debug, Clone)]
pub struct DemoOptions {
    pub now_ms: i64,
    pub stress_people: usize,
    pub tz: TimeZone,
}

impl DemoOptions {
    pub fn new(now_ms: i64) -> Self {
        Self { now_ms, stress_people: 0, tz: TimeZone::system() }
    }
}

enum At {
    Day(i64, &'static str),
    Mins(i64),
}

#[derive(Clone, Copy)]
enum Flavor {
    Normal,
    Automated,
    Newsletter,
}

struct File(&'static str, i64);

struct Person {
    key: &'static str,
    name: &'static str,
    email: &'static str,
    title: &'static str,
    org: &'static str,
    desc: &'static str,
    pinned: bool,
    ink: Option<u8>,
}

struct Msg {
    from: &'static str,
    at: At,
    text: &'static str,
    html: Option<&'static str>,
    files: Vec<File>,
    unread: bool,
}

struct Thread {
    subject: &'static str,
    with: Vec<&'static str>,
    flavor: Flavor,
    msgs: Vec<Msg>,
}

fn d(days: i64, at: &'static str) -> At {
    At::Day(days, at)
}

fn ago(mins: i64) -> At {
    At::Mins(mins)
}

fn m(from: &'static str, at: At, text: &'static str) -> Msg {
    Msg { from, at, text, html: None, files: Vec::new(), unread: false }
}

impl Msg {
    fn files(mut self, files: Vec<File>) -> Self {
        self.files = files;
        self
    }
    fn unread(mut self) -> Self {
        self.unread = true;
        self
    }
    fn html(mut self, html: &'static str) -> Self {
        self.html = Some(html);
        self
    }
}

fn t(subject: &'static str, with: &[&'static str], msgs: Vec<Msg>) -> Thread {
    Thread { subject, with: with.to_vec(), flavor: Flavor::Normal, msgs }
}

fn p(key: &'static str, name: &'static str, email: &'static str, title: &'static str, org: &'static str) -> Person {
    Person { key, name, email, title, org, desc: "", pinned: false, ink: None }
}

impl Person {
    fn desc(mut self, desc: &'static str) -> Self {
        self.desc = desc;
        self
    }
    fn pin(mut self) -> Self {
        self.pinned = true;
        self
    }
    fn ink(mut self, ink: u8) -> Self {
        self.ink = Some(ink);
        self
    }
}

#[rustfmt::skip]
fn known_people() -> Vec<Person> {
    vec![
        p("maya", "Maya Okafor", "maya.okafor@northwind.example", "Design lead", "Northwind")
            .desc("Runs the Q3 launch. Answers fastest before 10.")
            .pin()
            .ink(6),
        p("dad", "Dad", "pete.harlan@mail.example", "Family", "").pin().ink(1),
        p("dan", "Dan Whitaker", "dan.whitaker@northwind.example", "Engineering", "Northwind")
            .desc("Owns the launch build and the screenshots.")
            .ink(3),
        p("rasmus", "Rasmus Lindqvist", "rasmus@lindqvist-moe.example", "Accountant", "Lindqvist & Moe")
            .desc("Files VAT on the 27th. Do not send invoices after the 25th.")
            .ink(4),
        p("dhl", "DHL Parcel", "noreply@parcel.dhl.example", "Parcel updates", "").ink(2),
        p("amelie", "Amélie Dubois-Castellanos", "amelie@lepetitzinc.example", "Maîtresse d’hôtel", "Le Petit Zinc").ink(9),
        p("hiroshi", "田中 弘", "tanaka@kobo.example", "Translator", "Kobo Studio").ink(5),
        p("ali", "علي حسن", "ali.hassan@mail.example", "Landlord", "").ink(3),
        p("noa", "Noa Katz (נועה כץ)", "noa@mail.example", "Climbing partner", "").ink(7),
        p("sofia", "Sofia 🎉 Marchetti", "sofia@marchetti.example", "Events", "Studio Marchetti").ink(0),
        p("dispatch", "The Weekly Dispatch", "hello@weeklydispatch.example", "Newsletter", "").ink(6),
        p("payroll", "Northwind Payroll", "payroll@northwind.example", "Payslips", "").ink(1),
        p(
            "bartholomew",
            "Dr. Bartholomew Fitzgerald-Montgomery-Wellington III, Esq., of Fitzgerald, Montgomery & Wellington International Holdings LLP",
            "b.fitzgerald-montgomery-wellington@fmw-international-holdings.example",
            "Senior partner and head of cross-border commercial transactions",
            "Fitzgerald, Montgomery & Wellington International Holdings LLP",
        )
        .ink(2),
        p("jules", "Jules Moreau", "jules@mail.example", "Book club", "").ink(5),
        p("tomas", "Tomás Herrera", "tomas@cicloshop.example", "Bike shop", "").ink(1),
        p("wei", "Wei Zhang", "wei.zhang@mail.example", "Flatmate", "").ink(4),
        p("ingrid", "Ingrid Solberg", "ingrid@solberg.example", "Photographer", "").ink(6),
        p("kwame", "Kwame Mensah", "kwame@mail.example", "Former colleague", "").ink(2),
        p("chloe", "Chloé Bernard", "chloe@mail.example", "Carpool", "").ink(8),
        p("aiko", "Aiko Watanabe", "aiko@mail.example", "Conference co-chair", "").ink(0),
        p("mateo", "Mateo Rossi", "mateo@mail.example", "Neighbour", "").ink(9),
        p("hanna", "Hanna Berg", "reception@bergtandvard.example", "Dentist", "Berg Tandvård").ink(3),
        p("olu", "Olu Adeyemi", "olu@mail.example", "Friend", "").ink(7),
    ]
}

#[rustfmt::skip]
fn new_senders() -> Vec<Person> {
    vec![
        p("priya", "Priya Raman", "priya@parley.example", "Intro from Dan", "Parley").ink(8),
        p("marcus", "Marcus Lee", "marcus@harborroasters.example", "Sales", "Harbor Coffee Roasters").ink(1),
        p("ioana", "Ioana Popescu", "ioana.popescu@talentbridge.example", "Recruiter", "TalentBridge").ink(5),
        p("jonas", "Jonas Weber", "jonas@frankfurt-tech-meetup.example", "Organiser", "Frankfurt Tech Meetup").ink(3),
        p("elena", "Dr. Elena Fischer", "e.fischer@klinik-nord.example", "", "Klinik Nord").ink(9),
    ]
}

const DHL_HTML: &str = r##"<div style="font-family: Arial, sans-serif; max-width: 560px; margin: 0 auto; color: #222">
<img src="cid:dhl-logo" alt="DHL" width="96" style="display:block;margin:12px 0">
<h2 style="margin:0 0 8px;color:#d40511">Your parcel is on its way</h2>
<p>Your order <b>394-2211</b> has been handed to DHL.</p>
<table width="100%" cellpadding="6" style="border-collapse:collapse;border:1px solid #ddd">
<tr><td style="background:#f5f5f5"><b>Item</b></td><td>Xbox Series X, Galaxy Black (1 item)</td></tr>
<tr><td style="background:#f5f5f5"><b>Expected</b></td><td>Tuesday between 13:00 and 17:00</td></tr>
<tr><td style="background:#f5f5f5"><b>Tracking</b></td><td>JD014600003394512</td></tr>
</table>
<p><a href="https://tracking.parcel.dhl.example/?id=JD014600003394512&utm_source=email&utm_campaign=shipping" style="display:inline-block;background:#ffcc00;color:#222;padding:10px 18px;text-decoration:none;font-weight:bold">Track your parcel</a></p>
<img src="https://cdn.parcel.dhl.example/banner/summer.jpg" alt="Summer shipping offers" width="520">
<img src="https://pixel.mailtrack.io/open?id=88f1" width="1" height="1" alt="">
<script>alert('hello from a marketing email')</script>
</div>"##;

const DISPATCH_HTML: &str = r##"<div style="font-family: Georgia, serif; max-width: 600px; margin: 0 auto; color: #222; line-height: 1.55">
<h1 style="font-size: 26px">The case for boring infrastructure</h1>
<p><i>Issue 212</i></p>
<p>Most outages we read about this year had a boring cause. A certificate that expired, a disk that filled, a cron job nobody owned. The interesting failures are rare. The dull ones are weekly.</p>
<p>A useful exercise: list every system in your company that has <b>no named owner</b>. Not no documentation, no owner. In the three teams we asked, the list was longer than anyone expected, and two of the items were on the critical path for billing.</p>
<h3>This week's reading</h3>
<ul>
<li><a href="https://example.org/postmortem-unrotated-key?utm_source=dispatch&utm_medium=email&utm_campaign=212">A payments company traces a four-hour incident to one unrotated key</a></li>
<li><a href="https://example.org/runbooks-decay?utm_source=dispatch&fbclid=IwAR0abc">Why runbooks decay faster than code</a></li>
<li><a href="https://example.org/no-pager-in-two-years?utm_source=dispatch">An SRE who has not been paged at night in two years</a></li>
</ul>
<img src="https://img.weeklydispatch.example/issue-212-hero.png" alt="Illustration: a quiet server room" width="560">
<p style="font-size:12px;color:#888">You are receiving this because you subscribed. <a href="https://weeklydispatch.example/unsubscribe?u=77">Unsubscribe</a></p>
<img src="https://t.weeklydispatch.example/open.gif?u=77&i=212" width="1" height="1" alt="">
</div>"##;

const PAYROLL_HTML: &str = r##"<div style="font-family: Arial, sans-serif; color:#222"><p>Hello,</p><p>Your payslip for <b>October</b> is ready. It is attached to this message and available in the HR portal at <a href="https://northwind-hr-login.example.net/verify">https://hr.northwind.example/payslips</a>.</p><p style="color:#666;font-size:12px">This is an automated message. Please do not reply.</p></div>"##;

#[rustfmt::skip]
fn known_threads() -> Vec<Thread> {
    let mut threads = vec![
        t("Q3 launch checklist", &["maya", "dan"], vec![
            m("maya", d(23, "09:12"), "Hi, attaching the checklist for the Q3 launch. Two blockers so far: the pricing page copy isn't signed off, and Dan still needs to send the final screenshots.\n\nCan you take a look before Thursday's sync? If anything is missing, add it straight into the doc.").files(vec![File("Q3-launch-checklist.pdf", 214_000)]),
            m("me", d(23, "11:40"), "Thanks Maya. I added three items under Support: macros, the status page note and the refund FAQ.\n\nI'm moving the review to Thursday 10:00 so Dan can join. On pricing, legal wants the annual-discount wording looked at again. I'll ping them today."),
            m("maya", d(23, "13:05"), "Thursday 10:00 works. I'll move the invite."),
            m("dan", d(22, "16:48"), "Screenshots are in the shared folder now (v3). The empty states are still placeholders, I'll swap them by Wednesday.").files(vec![File("screenshots-v3.zip", 18_400_000)]),
            m("me", d(22, "17:02"), "Great, thanks Dan."),
            m("maya", d(20, "10:30"), "Legal came back: the discount wording is fine as long as we add \"billed annually\" next to the price. I have updated the copy."),
            m("me", d(20, "10:41"), "Done on my side too."),
            m("dan", d(19, "09:15"), "Empty states are in. Two of them are wider than the mockups, so check the onboarding screen in a small window."),
            m("maya", d(3, "16:20"), "Quick follow-up now that we have launched. Support is getting a lot of \"where is the annual toggle\" questions. Can we add a tooltip?"),
            m("me", d(3, "17:05"), "Good catch. I'll file it. It is probably a one-line change."),
            m("maya", d(2, "09:30"), "Also: Rasmus asked whether we can send the invoice batch earlier this month. I'll loop him in separately."),
            m("me", d(1, "08:10"), "Sounds good. I'll check with him."),
            m("maya", ago(300), "Tooltip is on staging. Could you try it on your phone and tell me if it overlaps the price?").files(vec![File("tooltip-staging.png", 88_000)]),
            m("maya", ago(38), "Never mind, I see it. It overlaps on narrow screens. Fixing now.").unread(),
        ]),
        t("Invoice 2291", &["maya"], vec![
            m("maya", d(9, "15:02"), "Attaching invoice 2291 for the design sprint. Net 30, as agreed.").files(vec![File("Invoice-2291.pdf", 96_000)]),
            m("me", d(9, "16:30"), "Received, thank you. I'll pass it to Rasmus today."),
            m("maya", d(8, "09:10"), "Great. Let me know if he needs the PO number."),
        ]),
        t("Offsite venues", &["maya"], vec![
            m("me", d(31, "10:00"), "Three options for the offsite:\n- The boathouse: 40 people, quiet\n- The old cannery: 60 people, loud but cheap\n- The monastery garden: 30 people, needs a six-week hold\n\nMy vote is the boathouse."),
            m("maya", d(31, "10:42"), "Boathouse. Can you check whether they allow outside catering? Last time we were charged a corkage fee."),
            m("me", d(30, "08:15"), "They allow it with a flat fee of 180. Cheaper than their catering."),
            m("maya", d(30, "09:00"), "Book it."),
            m("me", d(30, "09:12"), "Booked for the 14th. Confirmation attached.").files(vec![File("Boathouse-confirmation.pdf", 41_000)]),
        ]),
        t("Coffee Thursday?", &["maya"], vec![
            m("maya", d(1, "18:20"), "Coffee Thursday? There's a place near the station that opened last week."),
            m("me", d(1, "18:45"), "Thursday works. 9:30?"),
        ]),
        t("dinner sat?", &["dad"], vec![
            m("dad", d(1, "20:10"), "Are you coming Saturday? Mum is making the lamb."),
            m("me", d(1, "20:40"), "Probably yes. What time?"),
            m("dad", ago(125), "Seven. Bring the good bread from that bakery.").unread(),
        ]),
        t("Photos from the lake", &["dad"], vec![
            m("dad", d(12, "21:03"), "Found these on the old camera. Look at your haircut.").files(vec![File("lake-01.jpg", 2_400_000), File("lake-02.jpg", 2_100_000), File("lake-03.jpg", 1_900_000)]),
            m("me", d(12, "21:30"), "Ha. Frame the third one."),
        ]),
        t("Can you look at my phone?", &["dad"], vec![
            m("dad", d(40, "10:15"), "My phone says storage full again. I haven't taken any photos."),
            m("me", d(40, "12:00"), "Send me a screenshot of Settings, then Storage."),
            m("dad", d(40, "12:20"), "Attached.").files(vec![File("Screenshot.png", 412_000)]),
            m("me", d(40, "18:05"), "It's WhatsApp. 11 GB of videos. I'll show you on Saturday."),
        ]),
        Thread { subject: "Your parcel is on its way", with: vec!["dhl"], flavor: Flavor::Automated, msgs: vec![
            m("dhl", d(2, "08:01"), "Your order 394-2211 has been handed to DHL.\n\nXbox Series X, Galaxy Black (1 item)\nExpected delivery: Tuesday between 13:00 and 17:00\nTracking number: JD014600003394512").html(DHL_HTML).files(vec![File("dhl-logo.svg", 2_100)]),
            m("dhl", ago(95), "Out for delivery today. Your driver is 6 stops away.").unread(),
        ]},
        Thread { subject: "Delivery notification for order 288-1043", with: vec!["dhl"], flavor: Flavor::Automated, msgs: vec![
            m("dhl", d(35, "14:22"), "Your parcel was delivered to the neighbour at number 14. Signed for by M. Rossi."),
        ]},
        t("Invoice batch for October", &["rasmus"], vec![
            m("me", d(6, "09:05"), "Hi Rasmus, can we send the October batch on the 25th instead of the 30th? Two clients asked."),
            m("rasmus", d(6, "13:40"), "Should be possible. The only issue is the VAT filing, which I do on the 27th. Let me check the calendar."),
            m("rasmus", d(5, "08:20"), "Fine to move. I'll prepare the drafts for the 24th so you can review them first."),
        ]),
        t("Tax return questions", &["rasmus"], vec![
            m("rasmus", d(50, "15:00"), "A few questions about the home office deduction. Do you use the room only for work, and is there a separate meter?\n\nIf you can send the floor plan I can calculate the share.").files(vec![File("questions.pdf", 58_000)]),
            m("me", d(49, "19:30"), "Only for work, no separate meter. Floor plan attached.").files(vec![File("floor-plan.png", 640_000)]),
        ]),
        t("Réservation pour samedi", &["amelie"], vec![
            m("amelie", d(4, "10:22"), "Bonjour, nous avons bien noté votre réservation pour quatre personnes samedi à 20 h. Souhaitez-vous la table près de la fenêtre ?"),
            m("me", d(4, "11:05"), "Oui, merci ! Et nous aurons peut-être un cinquième invité. Je confirme demain."),
            m("amelie", d(4, "11:30"), "Parfait. Nous gardons la grande table au cas où."),
        ]),
        t("来週の打ち合わせ", &["hiroshi"], vec![
            m("hiroshi", d(8, "09:40"), "お世話になっております。来週の打ち合わせですが、火曜日の午後はいかがでしょうか。翻訳の初稿は月曜までにお送りします。"),
            m("me", d(8, "10:15"), "火曜日の午後で大丈夫です。初稿を楽しみにしています。"),
        ]),
        t("عقد الإيجار", &["ali"], vec![
            m("ali", d(15, "14:00"), "السلام عليكم، أرفقت لك نسخة عقد الإيجار الجديد. يرجى التوقيع وإعادته قبل نهاية الشهر.").files(vec![File("lease-2027.pdf", 330_000)]),
            m("me", d(15, "16:30"), "وعليكم السلام، استلمت العقد وسأعيده موقعًا هذا الأسبوع."),
        ]),
        t("Saturday climb?", &["noa"], vec![
            m("noa", d(2, "20:15"), "Saturday climb? I want to try the overhang again. Forecast says dry until 3.\nשבת בבוקר? אני יכולה להביא את החבל."),
            m("me", d(2, "20:44"), "Yes. 8:30 at the gym car park?"),
            m("noa", d(2, "20:50"), "מצוין. See you there."),
        ]),
        t("Launch party 🎉 RSVP", &["sofia"], vec![
            m("sofia", d(5, "12:00"), "We're doing a small launch party next Friday, 18:00 on the rooftop. Can you make it? Plus one welcome."),
            m("me", d(5, "12:30"), "Count me in, plus one."),
        ]),
        Thread { subject: "Issue 212: The case for boring infrastructure", with: vec!["dispatch"], flavor: Flavor::Newsletter, msgs: vec![
            m("dispatch", d(1, "07:00"), "This week: why the teams that ship calmly tend to run the least exciting stack.\n\nMost outages we read about this year had a boring cause. A certificate that expired, a disk that filled, a cron job nobody owned. The interesting failures are rare. The dull ones are weekly.\n\nA useful exercise: list every system in your company that has no named owner. Not no documentation, no owner.\n\nSee you next Thursday.").html(DISPATCH_HTML).unread(),
        ]},
        Thread { subject: "Your payslip for October", with: vec!["payroll"], flavor: Flavor::Automated, msgs: vec![
            m("payroll", d(18, "06:00"), "Your payslip for October is ready. It is attached to this message and available in the HR portal.").html(PAYROLL_HTML).files(vec![File("payslip-2026-10.pdf", 72_000)]),
        ]},
        t("Re: Re: Re: Fwd: Matter 2291-B / Heads of terms (privileged and confidential)", &["bartholomew"], vec![
            m("bartholomew", d(6, "17:45"), "Further to our call, please find the revised heads of terms attached. Kindly review clauses 4.2 and 7 in particular, as these reflect the position agreed between the parties at the meeting held on the 3rd.").files(vec![File("heads-of-terms-v7-clean-FINAL-reviewed-2.docx", 112_000)]),
            m("me", d(6, "18:10"), "Received. I'll revert tomorrow."),
        ]),
    ];

    struct Filler {
        key: &'static str,
        subject: &'static str,
        text: &'static str,
        reply: Option<&'static str>,
        days: i64,
        unread: bool,
    }
    let f = |key, subject, text, reply, days, unread| Filler { key, subject, text, reply, days, unread };
    let filler = [
        f("jules", "Book club on the 12th", "Are we still doing the book club on the 12th? I haven't finished the last third yet, so no spoilers.", Some("Still on. I will bring the cheese."), 7, false),
        f("tomas", "Bike repair", "Your bike is ready. The rear derailleur needed a new cable, so it is 28 instead of the estimate.", None, 4, true),
        f("wei", "Lease question", "Quick question about the lease: does the notice period start from the date I write or the date you receive it?", Some("From the date it is received, I think. Let me check the contract tonight."), 11, false),
        f("ingrid", "Photos from the wedding", "I finally went through the photos. Sending the good ones in batches so the email does not bounce.", None, 19, false),
        f("kwame", "Reference for Mira", "Would you be willing to be a reference for Mira? It would only take a short call.", Some("Happy to. Send her my number."), 26, false),
        f("chloe", "Re: carpool", "Thursday works. I can drive in the morning and you take the evening.", Some("Deal."), 3, false),
        f("aiko", "Conference talk", "The CFP closes Friday. Do you want to co-present the migration story?", None, 6, false),
        f("mateo", "Parcel pickup", "I have a parcel for you. I'll leave it with the neighbour at 14 if you're out.", Some("Thanks, I am out until six."), 35, false),
        f("hanna", "Dentist reminder", "A reminder that your appointment is on Monday at 08:30. Please arrive ten minutes early.", None, 2, true),
        f("olu", "Recipe", "Here's the recipe. The trick is to toast the spices before adding the tomatoes.", Some("Made it last night. Everyone asked for seconds."), 14, false),
    ];
    for Filler { key, subject, text, reply, days, unread } in filler {
        let mut msgs = vec![Msg { from: key, at: At::Day(days, "10:15"), text, html: None, files: Vec::new(), unread }];
        if let Some(reply) = reply {
            msgs.push(m("me", d(days, "11:02"), reply));
        }
        threads.push(Thread { subject, with: vec![key], flavor: Flavor::Normal, msgs });
    }
    threads
}

#[rustfmt::skip]
fn new_sender_threads() -> Vec<Thread> {
    vec![
        t("Intro: Priya <> you", &["priya"], vec![
            m("priya", d(3, "11:10"), "Hi, Dan suggested I write. I'm building a translation tool for support teams and would love 20 minutes of your time to hear how you handle multilingual tickets.\n\nNo slides, just questions. Any day next week works for me.").unread(),
        ]),
        t("Fresh-roasted beans for the office?", &["marcus"], vec![
            m("marcus", d(2, "09:03"), "Hello, I'm Marcus from Harbor Coffee Roasters. We supply small offices across the region and I thought your team might like a free tasting box. Reply with a delivery address and I will send one this week.").unread(),
        ]),
        t("Senior engineer role, remote (EU)", &["ioana"], vec![
            m("ioana", d(4, "15:20"), "Hi, I came across your profile and a client of ours is hiring a senior engineer, fully remote within the EU. Compensation is well above market. Would you be open to a short call this week?").unread(),
        ]),
        t("Speaker slot at the March meetup", &["jonas"], vec![
            m("jonas", d(6, "12:45"), "Hi! We run the Frankfurt Tech Meetup (about 200 members). We are looking for speakers for March and a colleague recommended you. Talks are 25 minutes, and we cover travel.").unread(),
        ]),
        t("Ihr Termin am 24. November", &["elena"], vec![
            m("elena", d(1, "08:30"), "Guten Tag, hiermit bestätigen wir Ihren Termin am 24. November um 10:15 Uhr. Bitte bringen Sie Ihre Versichertenkarte mit.").unread(),
        ]),
    ]
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() * items.len() as f64) as usize % items.len()]
    }
}

const STANDUP: &[&str] = &[
    "Blocked on the migration, waiting for the DBA to approve the plan.",
    "Merged the retry fix. Watching the error rate for an hour.",
    "Can someone review #4821? It is small, I promise.",
    "Out Thursday afternoon, back Friday.",
    "The flaky test is flaky again. I will quarantine it and open a ticket.",
    "Staging is down for the certificate renewal until 14:00.",
    "Shipped the export fix. Support can close the three linked tickets.",
    "I need a decision on the pagination format before I can continue.",
    "Pairing with Maya on the onboarding screen this morning.",
    "The vendor still has not replied about the API limits.",
    "Rolled back the cache change. Latency is back to normal.",
    "Notes from the call are in the doc. Two follow-ups are assigned to me.",
    "Can we move standup to 09:45? The train gets in late on Tuesdays.",
    "Dependency bump merged. No behaviour change expected.",
    "Found the cause: a timezone assumption in the nightly job.",
    "Let us not reopen the naming discussion this week.",
    "Done. Linked the PR in the ticket.",
    "I will take the on-call swap with Priya for the 14th.",
    "Planning poker is tomorrow. Please add your estimates beforehand.",
    "The dashboard is wrong for EU customers only. Investigating.",
];

const SUBJECT_POOL: &[&str] = &[
    "Quick question",
    "Re: Thursday",
    "Notes from today",
    "Can you send the file again?",
    "Updated draft",
    "Invoice for November",
    "Re: moving the call",
    "Photos",
    "Welcome aboard",
    "Following up",
    "Two small things",
    "Tickets for the 9th",
    "Re: the contract",
    "Venue options",
    "Reminder: renewal",
    "Draft agenda",
];

const TEXT_POOL: &[&str] = &[
    "Thanks, that helps. I will look at it this evening.",
    "Sending the updated version now. Let me know what you think.",
    "Can we talk on Thursday instead? Mornings are better for me.",
    "I looked into it and the numbers match. No action needed.",
    "Sorry for the late reply, it has been a hectic week.",
    "Attached is the final version. Please check the second page.",
    "That works for me. I will put it in the calendar.",
    "Could you confirm the address one more time?",
    "Good news: it was approved this morning.",
    "Let me know if you need anything else from my side.",
];

const FIRST: &[&str] = &[
    "Aaliyah", "Bruno", "Camille", "Dmitri", "Elena", "Farid", "Greta", "Hugo", "Isabel", "Jonas", "Keiko", "Lars",
    "Mina", "Nikolai", "Oksana", "Pavel", "Quinn", "Rosa", "Stefan", "Tamar", "Uma", "Viktor", "Wanjiru", "Ximena",
    "Yusuf", "Zainab", "Anders", "Beatriz", "Callum", "Dalia",
];

const LAST: &[&str] = &[
    "Abara",
    "Brandt",
    "Castillo",
    "Dvorak",
    "Eriksen",
    "Fontaine",
    "Gallagher",
    "Haddad",
    "Ivanova",
    "Jansen",
    "Kowalski",
    "Lindgren",
    "Moretti",
    "Nakamura",
    "Okoye",
    "Petrov",
    "Quispe",
    "Rahimi",
    "Sandoval",
    "Tran",
    "Ueda",
    "Vasquez",
    "Weber",
    "Yilmaz",
    "Zielinski",
    "Abernathy",
    "Bianchi",
];

const NATIVE: &[&str] = &[
    "佐藤 美咲",
    "李 明",
    "김 민준",
    "Алексей Смирнов",
    "Ελένη Παπά",
    "محمد الفاسي",
    "שרה לוי",
    "Åsa Öberg",
    "Łukasz Wróbel",
    "Đức Anh Nguyễn",
];

const TITLES: &[&str] = &[
    "Designer",
    "Engineer",
    "Recruiter",
    "Neighbour",
    "Client",
    "Teacher",
    "Landlord",
    "Doctor",
    "Friend from uni",
    "Conference speaker",
    "Plumber",
    "Editor",
];

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('.') && !out.is_empty() {
            out.push('.');
        }
    }
    out.trim_end_matches('.').to_string()
}

struct Seeder<'a> {
    store: &'a mut Store,
    account_id: i64,
    self_addresses: Vec<String>,
    opts: &'a DemoOptions,
    counter: u64,
}

impl Seeder<'_> {
    fn when(&self, at: &At) -> i64 {
        match at {
            At::Mins(m) => self.opts.now_ms - m * MINUTE,
            At::Day(days, hhmm) => {
                let (h, min) =
                    hhmm.split_once(':').map_or((0, 0), |(h, m)| (h.parse().unwrap_or(0), m.parse().unwrap_or(0)));
                let zoned = Timestamp::from_millisecond(self.opts.now_ms)
                    .map(|ts| ts.to_zoned_datetime(self.opts.tz.clone()))
                    .ok()
                    .and_then(|z| z.date().checked_sub(Span::new().days(*days)).ok())
                    .and_then(|date| date.at(h, min, 0, 0).to_zoned_datetime(self.opts.tz.clone()).ok());
                zoned.map_or(self.opts.now_ms - days * DAY, |z| z.timestamp().as_millisecond())
            }
        }
    }

    fn headers(flavor: Flavor, thread_key: &str) -> HeaderFacts {
        match flavor {
            Flavor::Normal => HeaderFacts::default(),
            Flavor::Automated => HeaderFacts { auto_submitted: Some("auto-generated".into()), ..Default::default() },
            Flavor::Newsletter => HeaderFacts {
                list_id: Some(format!("<{thread_key}.weeklydispatch.example>")),
                list_unsubscribe: Some(
                    "<https://weeklydispatch.example/unsubscribe?u=77>, <mailto:unsubscribe@weeklydispatch.example>"
                        .into(),
                ),
                list_unsubscribe_post: Some("List-Unsubscribe=One-Click".into()),
                precedence: Some("bulk".into()),
                ..Default::default()
            },
        }
    }

    fn address_of(people: &[&Person], key: &str) -> Address {
        people
            .iter()
            .find(|p| p.key == key)
            .map_or_else(|| Address::new(&format!("{key}@mail.example"), None), |p| Address::new(p.email, Some(p.name)))
    }

    fn build(&mut self, thread: &Thread, roster: &[&Person]) -> Vec<IncomingMessage> {
        self.counter += 1;
        let thread_id = format!("demo-t-{}", self.counter);
        let me = Address::new(&self.self_addresses[0], Some("You"));
        let others: Vec<Address> = thread.with.iter().map(|k| Self::address_of(roster, k)).collect();
        let mut out = Vec::new();
        for (i, msg) in thread.msgs.iter().enumerate() {
            let from_me = msg.from == "me";
            let from = if from_me { me.clone() } else { Self::address_of(roster, msg.from) };
            let to: Vec<Address> = if from_me {
                others.clone()
            } else {
                std::iter::once(me.clone()).chain(others.iter().filter(|a| a.email != from.email).cloned()).collect()
            };
            let mut attachments = Vec::new();
            for f in &msg.files {
                let inline = f.0 == "dhl-logo.svg";
                attachments.push(IncomingAttachment {
                    provider_id: None,
                    filename: f.0.to_string(),
                    mime: mime_of(f.0).to_string(),
                    size: f.1,
                    content_id: inline.then(|| "dhl-logo".to_string()),
                    inline,
                });
            }
            let quoted = if i > 0 && msg.text.len() > 20 {
                let prev = &thread.msgs[i - 1];
                let quote: String = prev
                    .text
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .take(3)
                    .map(|l| format!("> {l}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(format!(
                    "\n\nOn {}, {} wrote:\n{quote}",
                    "an earlier day",
                    if prev.from == "me" { "You" } else { prev.from }
                ))
            } else {
                None
            };
            let text = match &quoted {
                Some(q) => format!("{}{q}", msg.text),
                None => msg.text.to_string(),
            };
            out.push(IncomingMessage {
                provider_id: format!("{thread_id}-m{i}"),
                thread_id: thread_id.clone(),
                rfc_message_id: Some(format!("<{thread_id}.{i}@zsortmail.demo>")),
                in_reply_to: (i > 0).then(|| format!("<{thread_id}.{}@zsortmail.demo>", i - 1)),
                from,
                to,
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: thread.subject.to_string(),
                sent_at: self.when(&msg.at),
                snippet: None,
                text: Some(text),
                html: msg.html.map(str::to_string),
                attachments,
                labels: vec![if from_me { "SENT" } else { "INBOX" }.to_string()],
                unread: msg.unread,
                starred: false,
                headers: if from_me { HeaderFacts::default() } else { Self::headers(thread.flavor, &thread_id) },
                size_bytes: Some(msg.text.len() as i64 + 1800),
            });
        }
        out
    }

    fn ingest(&mut self, messages: Vec<IncomingMessage>, screener: bool) -> Result<()> {
        let ctx = IngestContext {
            account_id: self.account_id,
            self_addresses: self.self_addresses.clone(),
            screener,
            automation: false,
        };
        for chunk in messages.chunks(500) {
            self.store.ingest_batch(&ctx, chunk, self.opts.now_ms)?;
        }
        Ok(())
    }

    fn set_profile(&self, person: &Person) -> Result<()> {
        self.store.conn().execute(
            "UPDATE people SET title = ?1, organization = NULLIF(?2, ''), description = NULLIF(?3, ''), pinned = ?4,
                    ink = COALESCE(?5, ink)
             WHERE id = (SELECT person_id FROM person_addresses WHERE account_id = ?6 AND address = ?7)",
            rusqlite::params![
                person.title,
                person.org,
                person.desc,
                person.pinned,
                person.ink.map(i64::from),
                self.account_id,
                person.email.to_lowercase(),
            ],
        )?;
        let id: Option<i64> = self
            .store
            .conn()
            .query_row(
                "SELECT person_id FROM person_addresses WHERE account_id = ?1 AND address = ?2",
                rusqlite::params![self.account_id, person.email.to_lowercase()],
                |r| r.get(0),
            )
            .ok();
        if let Some(id) = id {
            crate::store::people::refresh_person(self.store.conn(), id)?;
        }
        Ok(())
    }

    fn long_thread(&mut self, dan: &Person) -> Result<()> {
        let mut rng = Rng(2291);
        let count = 300usize;
        let mut gaps: Vec<i64> = Vec::with_capacity(count);
        for _ in 1..count {
            let r = rng.next();
            gaps.push(if r < 0.6 {
                ((1.0 + rng.next() * 25.0) * MINUTE as f64) as i64
            } else if r < 0.85 {
                ((1.0 + rng.next() * 10.0) * HOUR as f64) as i64
            } else if r < 0.97 {
                ((1.0 + rng.next() * 4.0) * DAY as f64) as i64
            } else {
                ((8.0 + rng.next() * 20.0) * DAY as f64) as i64
            });
        }
        let mut times = vec![0i64; count];
        let mut t = self.opts.now_ms - 3 * DAY;
        for i in (0..count).rev() {
            times[i] = t;
            if i > 0 {
                t -= gaps[i - 1];
            }
        }
        self.counter += 1;
        let thread_id = format!("demo-t-{}", self.counter);
        let me = Address::new(&self.self_addresses[0], Some("You"));
        let them = Address::new(dan.email, Some(dan.name));
        let mut from_me = false;
        let mut msgs = Vec::with_capacity(count);
        let mut prev = String::new();
        for (i, at) in times.iter().enumerate() {
            if rng.next() < 0.6 {
                from_me = !from_me;
            }
            let paragraphs = if rng.next() < 0.08 { 2 } else { 1 };
            let body: String = (0..paragraphs).map(|_| *rng.pick(STANDUP)).collect::<Vec<_>>().join("\n\n");
            let text = if i > 0 && !prev.is_empty() {
                format!("{body}\n\nOn an earlier day, someone wrote:\n> {}", prev.lines().next().unwrap_or(""))
            } else {
                body.clone()
            };
            let attachments = if rng.next() < 0.05 {
                vec![IncomingAttachment {
                    provider_id: None,
                    filename: format!("log-{i}.png"),
                    mime: "image/png".into(),
                    size: 40_000 + (rng.next() * 400_000.0) as i64,
                    content_id: None,
                    inline: false,
                }]
            } else {
                Vec::new()
            };
            msgs.push(IncomingMessage {
                provider_id: format!("{thread_id}-m{i}"),
                thread_id: thread_id.clone(),
                rfc_message_id: None,
                in_reply_to: None,
                from: if from_me { me.clone() } else { them.clone() },
                to: vec![if from_me { them.clone() } else { me.clone() }],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Re: Re: Re: Fwd: team standup notes".into(),
                sent_at: *at,
                snippet: None,
                text: Some(text),
                html: None,
                attachments,
                labels: vec![if from_me { "SENT" } else { "INBOX" }.into()],
                unread: false,
                starred: false,
                headers: HeaderFacts::default(),
                size_bytes: Some(2_000),
            });
            prev = body;
        }
        self.ingest(msgs, false)
    }

    fn stress(&mut self, count: usize) -> Result<()> {
        let mut rng = Rng(7);
        let me = Address::new(&self.self_addresses[0], Some("You"));
        let mut batch: Vec<IncomingMessage> = Vec::new();
        for i in 0..count {
            let name = if rng.next() < 0.08 {
                (*rng.pick(NATIVE)).to_string()
            } else {
                format!("{} {}", rng.pick(FIRST), rng.pick(LAST))
            };
            let email = format!("{}.{i}@example.com", slug(&name).chars().take(24).collect::<String>());
            let them = Address::new(&email, Some(&name));
            for _ in 0..(1 + (rng.next() * 3.0) as usize) {
                self.counter += 1;
                let thread_id = format!("demo-t-{}", self.counter);
                let subject = (*rng.pick(SUBJECT_POOL)).to_string();
                let mut at = self.opts.now_ms - (rng.next() * 400.0) as i64 * DAY - (rng.next() * 20.0) as i64 * HOUR;
                for k in 0..(1 + (rng.next() * 5.0) as usize) {
                    let from_me = rng.next() < 0.5;
                    batch.push(IncomingMessage {
                        provider_id: format!("{thread_id}-m{k}"),
                        thread_id: thread_id.clone(),
                        rfc_message_id: None,
                        in_reply_to: None,
                        from: if from_me { me.clone() } else { them.clone() },
                        to: vec![if from_me { them.clone() } else { me.clone() }],
                        cc: Vec::new(),
                        bcc: Vec::new(),
                        subject: subject.clone(),
                        sent_at: at,
                        snippet: None,
                        text: Some((*rng.pick(TEXT_POOL)).to_string()),
                        html: None,
                        attachments: Vec::new(),
                        labels: vec![if from_me { "SENT" } else { "INBOX" }.into()],
                        unread: false,
                        starred: false,
                        headers: HeaderFacts::default(),
                        size_bytes: Some(1_500),
                    });
                    at += (5 + (rng.next() * 600.0) as i64) * MINUTE;
                }
            }
            let _ = (TITLES.len(), ink_for(&email));
            if batch.len() >= 1_000 {
                self.ingest(std::mem::take(&mut batch), false)?;
            }
        }
        self.ingest(batch, false)
    }
}

fn mime_of(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("zip") => "application/zip",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        _ => "application/octet-stream",
    }
}

pub fn seed_demo(store: &mut Store, opts: &DemoOptions) -> Result<i64> {
    let account_id = store.upsert_account("demo", DEMO_EMAIL, Some("Demo mailbox"), opts.now_ms)?;
    store.set_setting(&format!("screener.{account_id}"), "1")?;
    let known = known_people();
    let new = new_senders();
    let roster: Vec<&Person> = known.iter().chain(new.iter()).collect();

    let mut seeder = Seeder { store, account_id, self_addresses: vec![DEMO_EMAIL.to_string()], opts, counter: 0 };
    let mut messages = Vec::new();
    for thread in known_threads() {
        messages.extend(seeder.build(&thread, &roster));
    }
    seeder.ingest(messages, false)?;

    let mut waiting = Vec::new();
    for thread in new_sender_threads() {
        waiting.extend(seeder.build(&thread, &roster));
    }
    seeder.ingest(waiting, true)?;

    if let Some(dan) = known.iter().find(|p| p.key == "dan") {
        seeder.long_thread(dan)?;
    }
    for person in known.iter().chain(new.iter()) {
        seeder.set_profile(person)?;
    }
    if opts.stress_people > 0 {
        seeder.stress(opts.stress_people)?;
    }
    Ok(account_id)
}

pub const SECOND_DEMO_EMAIL: &str = "work@demo.example";

pub fn seed_second_demo(store: &mut Store, opts: &DemoOptions) -> Result<i64> {
    if let Some(existing) = store.accounts()?.into_iter().find(|a| a.email == SECOND_DEMO_EMAIL) {
        return Ok(existing.id);
    }
    let account_id = store.upsert_account("demo", SECOND_DEMO_EMAIL, Some("Work mailbox"), opts.now_ms)?;
    store.set_setting(&format!("screener.{account_id}"), "1")?;
    let people = vec![
        p("wanda", "Wanda Whitfield", "wanda@acme.example", "Project manager", "Acme").ink(5),
        p("theo", "Theo Ledger", "theo@acme.example", "Finance", "Acme").ink(8),
    ];
    let roster: Vec<&Person> = people.iter().collect();
    let mut seeder =
        Seeder { store, account_id, self_addresses: vec![SECOND_DEMO_EMAIL.to_string()], opts, counter: 5_000 };
    let threads = vec![
        t(
            "Budget review",
            &["wanda"],
            vec![
                m("wanda", d(4, "09:00"), "Can you send the Q4 budget numbers before Friday?"),
                m("me", d(4, "10:15"), "Yes, I will have them ready on Thursday."),
            ],
        ),
        t("Invoice 77", &["theo"], vec![m("theo", d(1, "14:30"), "Invoice 77 is approved and goes out tomorrow.")]),
    ];
    let mut messages = Vec::new();
    for thread in threads {
        messages.extend(seeder.build(&thread, &roster));
    }
    seeder.ingest(messages, false)?;
    for person in &people {
        seeder.set_profile(person)?;
    }
    Ok(account_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{ConversationStatus, PeopleFilter, PersonKind, PersonStatus};

    const NOW: i64 = 1_795_082_400_000;

    fn seeded() -> (Store, DemoOptions) {
        let mut store = Store::open_in_memory().unwrap();
        let opts = DemoOptions { now_ms: 1_795_082_400_000, stress_people: 0, tz: TimeZone::UTC };
        seed_demo(&mut store, &opts).unwrap();
        (store, opts)
    }

    fn person(store: &Store, query: &str) -> crate::dto::PersonSummary {
        store
            .list_people(PeopleFilter::All, query)
            .unwrap()
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("no one matches {query}"))
    }

    #[test]
    fn the_second_mailbox_is_small_separate_and_safe_to_seed_twice() {
        use crate::dto::PeopleFilter;
        let (mut store, opts) = seeded();
        let everyone = store.list_people(PeopleFilter::All, "").unwrap().len();
        let second = seed_second_demo(&mut store, &opts).unwrap();
        assert_eq!(seed_second_demo(&mut store, &opts).unwrap(), second);
        assert_eq!(store.accounts().unwrap().len(), 2);
        let all: Vec<String> = store.list_people(PeopleFilter::All, "").unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(all.len(), everyone + 2, "{all:?}");
        let _ = &all;
        store.set_account_scope(Some(second));
        let names: Vec<String> =
            store.list_people(PeopleFilter::All, "").unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.iter().any(|n| n.starts_with("Wanda")));
        assert_eq!(store.inbox(None, opts.now_ms, 50, 0).unwrap().len(), 2);
        assert_eq!(store.default_account().unwrap(), second, "a profile writes from its own mailbox");
        let wanda = store.suggest_recipients("wanda", 8).unwrap();
        assert_eq!(wanda.len(), 1, "a profile suggests its own contacts: {wanda:?}");
        assert!(store.suggest_recipients("okafor", 8).unwrap().is_empty(), "not another mailbox's");
        store.set_account_scope(None);
        store
            .conn()
            .execute(
                "INSERT INTO people (account_id, kind, status, display_name, ink, created_at, last_message_at) VALUES (?1, 'person', 'known', 'Maya (work)', 1, 0, 1)",
                [second],
            )
            .unwrap();
        let twin = store.conn().last_insert_rowid();
        store
            .conn()
            .execute("INSERT INTO person_addresses (account_id, address, person_id, is_primary) VALUES (?1, 'maya.okafor@northwind.example', ?2, 1)", rusqlite::params![second, twin])
            .unwrap();
        let found = store.suggest_recipients("maya.okafor", 8).unwrap();
        assert_eq!(found.iter().filter(|s| s.email == "maya.okafor@northwind.example").count(), 1, "{found:?}");
    }

    #[test]
    fn seeds_the_whole_mailbox() {
        let (store, _) = seeded();
        let people = store.list_people(PeopleFilter::All, "").unwrap();
        assert_eq!(people.len(), 23, "known people only; the screener holds the rest");
        let accounts = store.accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].provider, "demo");
    }

    #[test]
    fn lists_pinned_people_first_and_then_by_recency() {
        let (store, _) = seeded();
        let people = store.list_people(PeopleFilter::All, "").unwrap();
        let pinned = people.iter().take_while(|p| p.pinned).map(|p| p.name.as_str()).collect::<Vec<_>>();
        assert_eq!(pinned, ["Maya Okafor", "Dad"]);
        let rest = &people[pinned.len()..];
        assert!(rest.windows(2).all(|w| w[0].last_at >= w[1].last_at));
    }

    #[test]
    fn people_search_ignores_accents_and_case_and_covers_titles_and_addresses() {
        let (store, _) = seeded();
        assert_eq!(person(&store, "AMELIE").name, "Amélie Dubois-Castellanos");
        assert_eq!(store.list_people(PeopleFilter::All, "tomas").unwrap().len(), 1);
        assert!(store.list_people(PeopleFilter::All, "northwind").unwrap().len() >= 3);
        assert_eq!(store.list_people(PeopleFilter::All, "rasmus@lindqvist").unwrap().len(), 1);
        assert!(store.list_people(PeopleFilter::All, "checklist").unwrap().iter().any(|p| p.name == "Maya Okafor"));
        assert!(store.list_people(PeopleFilter::All, "xbox").unwrap().is_empty());
    }

    #[test]
    fn derives_needs_reply_and_waiting_on_them_from_who_wrote_last() {
        let (store, _) = seeded();
        let maya = person(&store, "Okafor");
        let convs = store.list_conversations(maya.id).unwrap();
        let by = |subject: &str| convs.iter().find(|c| c.subject == subject).unwrap_or_else(|| panic!("{subject}"));
        assert_eq!(by("Q3 launch checklist").status, ConversationStatus::NeedsReply);
        assert_eq!(by("Coffee Thursday?").status, ConversationStatus::WaitingOnThem);
        assert!(maya.needs_reply && maya.waiting_on_them && maya.has_files);
        assert_eq!(maya.unread, 1);
    }

    #[test]
    fn automated_and_newsletter_senders_are_classified_and_never_need_a_reply() {
        let (store, _) = seeded();
        let dhl = person(&store, "DHL");
        assert_eq!(dhl.kind, PersonKind::Service);
        assert!(!dhl.needs_reply);
        let dispatch = person(&store, "Dispatch");
        assert_eq!(dispatch.kind, PersonKind::List);
    }

    #[test]
    fn unknown_senders_wait_in_new_senders_instead_of_people() {
        let (store, _) = seeded();
        assert!(store.list_people(PeopleFilter::All, "Priya").unwrap().is_empty());
        let status: String = store
            .conn()
            .query_row("SELECT status FROM people WHERE display_name = 'Priya Raman'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(parse_status_for_test(&status), PersonStatus::NewSender);
        let waiting: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM people WHERE status = 'new_sender'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(waiting, 5);
    }

    fn parse_status_for_test(s: &str) -> PersonStatus {
        crate::store::people::parse_status(s)
    }

    #[test]
    fn a_group_conversation_shows_under_each_participant() {
        let (store, _) = seeded();
        let maya = person(&store, "Okafor");
        let dan = person(&store, "Whitaker");
        let from_maya = store
            .list_conversations(maya.id)
            .unwrap()
            .into_iter()
            .find(|c| c.subject == "Q3 launch checklist")
            .unwrap();
        let from_dan =
            store.list_conversations(dan.id).unwrap().into_iter().find(|c| c.subject == "Q3 launch checklist").unwrap();
        assert_eq!(from_maya.id, from_dan.id);
        assert_eq!(from_maya.others.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Dan Whitaker"]);
        assert_eq!(from_dan.others.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Maya Okafor"]);
    }

    #[test]
    fn a_conversation_reads_oldest_first_with_quoted_history_split_off() {
        let (store, _) = seeded();
        let maya = person(&store, "Okafor");
        let summary = store
            .list_conversations(maya.id)
            .unwrap()
            .into_iter()
            .find(|c| c.subject == "Q3 launch checklist")
            .unwrap();
        let detail = store.get_conversation(summary.id, Some(maya.id)).unwrap().unwrap();
        assert_eq!(detail.messages.len(), 14);
        let times: Vec<i64> = detail.messages.iter().map(|m| m.sent_at).collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]));
        assert!(detail.messages[0].quoted.is_none());
        let second = &detail.messages[1];
        assert!(second.quoted.as_deref().is_some_and(|q| q.contains("wrote:")), "history is stripped into `quoted`");
        assert!(!second.text.contains("wrote:"), "and is not in the text");
        assert!(detail.messages.last().unwrap().unread);
        assert_eq!(second.to_line, "to Maya and Dan");
        assert_eq!(detail.messages[0].to_line, "to you and Dan");
    }

    #[test]
    fn html_mail_is_sanitized_with_trackers_removed_and_remote_images_counted() {
        let (store, _) = seeded();
        let dhl = person(&store, "DHL");
        let summary = store
            .list_conversations(dhl.id)
            .unwrap()
            .into_iter()
            .find(|c| c.subject == "Your parcel is on its way")
            .unwrap();
        let detail = store.get_conversation(summary.id, Some(dhl.id)).unwrap().unwrap();
        let first = &detail.messages[0];
        assert_eq!(first.html_url.as_deref(), Some(format!("msg/{}", first.id).as_str()));
        assert_eq!(first.trackers_blocked, 1);
        assert_eq!(first.remote_images, 1);
        let html = store.message_html(first.id).unwrap().unwrap();
        assert!(!html.contains("<script") && !html.contains("alert("), "script removed");
        assert!(!html.contains("mailtrack"), "tracking pixel removed");
        assert!(html.contains("cid:dhl-logo"), "inline image kept for the protocol to serve");
        assert!(!html.contains("utm_source"), "tracking parameters stripped from links");
        let logo = store.attachment_by_content_id(first.id, "dhl-logo").unwrap().unwrap();
        assert!(logo.inline);
        assert!(
            first.attachments.iter().any(|a| a.inline),
            "the inline logo is flagged so the UI does not list it as a file"
        );
    }

    #[test]
    fn newsletters_expose_one_click_unsubscribe() {
        let (store, _) = seeded();
        let dispatch = person(&store, "Dispatch");
        let summary = store.list_conversations(dispatch.id).unwrap().remove(0);
        let detail = store.get_conversation(summary.id, Some(dispatch.id)).unwrap().unwrap();
        let unsubscribe = detail.messages[0].unsubscribe.as_ref().expect("unsubscribe info");
        assert!(unsubscribe.one_click);
        assert!(unsubscribe.https.as_deref().is_some_and(|u| u.starts_with("https://")));
        assert!(unsubscribe.mailto.is_some());
    }

    #[test]
    fn files_are_gathered_per_person_without_inline_images() {
        let (store, _) = seeded();
        let maya = person(&store, "Okafor");
        let files = store.list_files(maya.id).unwrap();
        assert_eq!(files.len(), 5);
        assert!(files.windows(2).all(|w| w[0].sent_at >= w[1].sent_at));
        let dhl = person(&store, "DHL");
        assert!(store.list_files(dhl.id).unwrap().is_empty(), "the DHL logo is inline, not a file");
    }

    #[test]
    fn the_long_thread_has_three_hundred_messages() {
        let (store, _) = seeded();
        let dan = person(&store, "Whitaker");
        let long = store
            .list_conversations(dan.id)
            .unwrap()
            .into_iter()
            .find(|c| c.message_count == 300)
            .expect("long thread");
        let detail = store.get_conversation(long.id, Some(dan.id)).unwrap().unwrap();
        assert_eq!(detail.messages.len(), 300);
    }

    #[test]
    fn message_search_finds_words_in_bodies_and_subjects_and_ignores_quoted_history() {
        let (store, _) = seeded();
        let xbox = store.search("xbox 394", NOW, 10).unwrap();
        assert!(xbox.messages.iter().any(|h| h.subject == "Your parcel is on its way"), "body words");
        let subject = store.search("offsite venues", NOW, 10).unwrap();
        assert!(!subject.messages.is_empty());
        let hits = store.search("corkage", NOW, 10).unwrap();
        assert_eq!(hits.messages.len(), 1, "only the message that wrote it, not the replies that quote it");
        assert!(store.search("", NOW, 10).unwrap().messages.is_empty());
    }

    #[test]
    fn stress_people_are_added_deterministically_and_stay_fast_to_list() {
        let mut store = Store::open_in_memory().unwrap();
        let opts = DemoOptions { now_ms: 1_795_082_400_000, stress_people: 600, tz: TimeZone::UTC };
        seed_demo(&mut store, &opts).unwrap();
        let started = std::time::Instant::now();
        let people = store.list_people(PeopleFilter::All, "").unwrap();
        assert!(people.len() >= 600 + 23);
        assert!(started.elapsed().as_millis() < 250, "listing took {:?}", started.elapsed());
    }
}
