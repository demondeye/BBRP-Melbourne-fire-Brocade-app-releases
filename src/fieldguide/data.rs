//! Field Guide content — from the "AV Even More Cheaty Sheet"
//! (compiled by Blaire, Flamingo and Leah). Edit the text here.
//!
//! A page has columns → sections → steps. Sections are tick-off checklists
//! unless made with `info(..)` (symptoms, warnings, reference lists).
//! Pages built with `choice(..)` ask which situation applies first.
//! The page `id` is what a Stream Deck key uses: /api/fieldguide/open/<id>

use std::sync::OnceLock;

#[derive(Clone)]
pub struct Section {
    pub title: &'static str,
    pub steps: Vec<&'static str>,
    pub info: bool,
    /// Some(true) = only shows when MICA is available, Some(false) = only when it isn't.
    pub mica: Option<bool>,
}

pub struct Column {
    pub sections: Vec<Section>,
}

pub struct Opt {
    pub name: &'static str,
    pub sections: Vec<Section>,
}

pub struct Table {
    pub title: &'static str,
    pub head: Vec<&'static str>,
    pub rows: Vec<Vec<&'static str>>,
}

pub enum Kind {
    Home,
    Columns(Vec<Column>),
    /// Only ONE situation applies: the user picks an option and only that
    /// option's steps are shown and counted. `intro` shows above the choice, `outro` below.
    Choice { question: &'static str, options: Vec<Opt>, intro: Vec<Section>, outro: Vec<Section> },
    Tables(Vec<Table>),
}

pub struct Page {
    pub id: &'static str,
    pub title: &'static str,
    pub group: &'static str,
    pub icon: &'static str,
    pub color: [u8; 3],
    pub kind: Kind,
}

impl Page {
    pub fn options(&self) -> Option<&Vec<Opt>> {
        match &self.kind {
            Kind::Choice { options, .. } => Some(options),
            _ => None,
        }
    }
}

pub struct Group {
    pub id: &'static str,
    pub name: &'static str,
}

pub const GROUPS: [Group; 3] = [
    Group { id: "injury", name: "What is the nature of the injury?" },
    Group { id: "reaction", name: "How are they reacting to it?" },
    Group { id: "reference", name: "Reference" },
];

fn sec(title: &'static str, steps: &[&'static str]) -> Section {
    Section { title, steps: steps.to_vec(), info: false, mica: None }
}
fn info(title: &'static str, steps: &[&'static str]) -> Section {
    Section { title, steps: steps.to_vec(), info: true, mica: None }
}
/// Shows when MICA is available.
fn if_mica(title: &'static str, steps: &[&'static str]) -> Section {
    Section { title, steps: steps.to_vec(), info: false, mica: Some(true) }
}
/// Shows when MICA is not available.
fn no_mica(title: &'static str, steps: &[&'static str]) -> Section {
    Section { title, steps: steps.to_vec(), info: false, mica: Some(false) }
}
fn col(sections: Vec<Section>) -> Column {
    Column { sections }
}
fn opt(name: &'static str, sections: Vec<Section>) -> Opt {
    Opt { name, sections }
}
fn page(id: &'static str, title: &'static str, group: &'static str, columns: Vec<Column>) -> Page {
    Page { id, title, group, icon: "", color: [0, 0, 0], kind: Kind::Columns(columns) }
}
fn choice(id: &'static str, title: &'static str, group: &'static str, question: &'static str, options: Vec<Opt>, intro: Vec<Section>, outro: Vec<Section>) -> Page {
    Page { id, title, group, icon: "", color: [0, 0, 0], kind: Kind::Choice { question, options, intro, outro } }
}

fn list() -> Vec<Page> {
    vec![
        choice("broken-bones", "Broken Bones / Dislocation", "injury", "What kind of injury is it?", vec![
            opt("Fracture", vec![sec("What to do", &["Fractures are partial/full cracks in the bone", "To treat apply a splint to the affected area", "Transport patient onto stretcher for patient comfort"])]),
            opt("Bad break", vec![sec("What to do", &["Bad breaks are when there are bends where there shouldn't be (legs and arms should not have three joints!)", "If they are conscious tell the patient that you will need to set the bone", "Set the bone, don't mind the screaming", "Apply splint to affected area", "Transport patient onto stretcher for patient comfort"])]),
            opt("Exposed bone", vec![sec("What to do", &["Exposed bone is anything perforating the skin", "Apply donut bandage around exposed bone", "Wrap with pressure bandage to keep area stable", "Transport patient onto stretcher for patient comfort"])]),
            opt("Broken rib", vec![sec("What to do", &["Not much can be done for broken ribs", "Treat for any bruising in the area and let the hospital help them", "Transport patient onto stretcher for patient comfort"])]),
            opt("Dislocation", vec![sec("What to do", &["A dislocation is a joint removed out of a socket", "Warn the patient that what you're doing will hurt", "Reassure them that it will stop hurting very soon", "Reseat the joint into the socket", "Transport patient onto stretcher for patient comfort"])]),
        ], vec![], vec![]),
        choice("drowning", "Drowning", "injury", "What do you find?", vec![
            opt("Water present - no breathing", vec![sec("What to do", &["Gently press on their midsection to remove water"])]),
            opt("No water present - breathing", vec![sec("What to do", &["Place oxygen mask on patient"])]),
        ],
        vec![sec("First steps", &["Place patient in recovery position", "Check for breathing", "Check airways - are they blocked?"])],
        vec![
            if_mica("Patient is still waterlogged", &["Call for MICA"]),
            no_mica("Patient is still waterlogged", &["Transport as critical"]),
        ]),
        choice("stabbing", "Stabbing / Shrapnel", "injury", "What kind of wound is it?", vec![
            opt("Minor stab wound", vec![sec("What to do", &["Clean the wound with saline", "Seal the wound with butterfly bandages", "Apply pressure bandage to wound if heavy bleeding"])]),
            opt("Stab to heart or main arteries", vec![
                if_mica("MICA", &["Call for MICA"]),
                sec("What to do", &["Monitor vitals closely", "Infuse IV fluids if BP drops", "Give patient betablockers/high pulse pain medication if BP rises"]),
                no_mica("No MICA", &["Stabilise patient to best ability", "Take patient to hospital as critical"]),
            ]),
            opt("Shrapnel on the surface", vec![sec("What to do", &["Clean the shrapnel out with saline", "Place bandage over area", "Apply pressure bandage to wound if heavy bleeding"])]),
            opt("Shrapnel embedded", vec![sec("What to do", &["Clean the area around the shrapnel", "Place donut bandage around shrapnel area", "Wrap around donut bandage"])]),
        ], vec![], vec![]),
        page("drug-od", "Drug Overdose", "injury", vec![col(vec![
            sec("Breathing", &["Move into recovery position", "Check breathing. If choking, clear airways"]),
            if_mica("MICA", &["Insert breathing tube if necessary - only MICA"]),
            sec("Treatment", &["Move into recovery position", "Take patient away from noise and distraction if necessary", "Give patient activated charcoal pills (absorbs the drugs/alcohol)", "Induce vomiting"]),
        ])]),
        choice("gunshot", "Gunshot Wounds", "injury", "Is there an exit wound?", vec![
            opt("Exit wound present", vec![sec("What to do", &["Clean area with saline", "Pack wound with gauze", "Wrap overtop with pressure bandage"])]),
            opt("No exit wound", vec![sec("What to do", &["Apply C collar and move patient onto spinal board to stabilize", "Reduce any movement so bullet does not move inside body", "Clean wound with saline", "Pack wound gently with gauze", "Wrap area lightly in a bandage to reduce bleeding"])]),
        ], vec![], vec![]),
        page("burns", "Burn Victims", "injury", vec![col(vec![
            sec("Treatment", &["Is there swelling to airways? (burns to face, mouth or throat)", "Apply cool water/saline to burns (there is water in the ambulance)", "Wrap burn in happy plastic (glad wrap)", "Tell patient not to move the burnt area as to not irritate the burn"]),
            sec("If airways blocked", &["Remove any blockages", "Apply oxygen mask to assist with breathing"]),
            if_mica("MICA", &["Breathing tube if oxygen mask fails to make patient breathe (MICA)"]),
            info("Burns pain management", &["Burns to the patient's airways - do not give oral pain medication", "Stick to IV pain relief as oral medication may irritate further"]),
        ])]),
        choice("blunt-force", "Blunt Force Trauma", "injury", "What happened?", vec![
            opt("Falls / MVA", vec![
                sec("Assess", &["Assess if patient has a spinal injury (if so, switch to Spinal patient)", "Check for patient awareness - if unconscious check vitals", "Check for any lacerations, contusions or protrusions (check for any cuts, bruises or breaks)"]),
                sec("Treatment", &["Clean any cuts with saline and bandage", "Check for broken bones - if broken, see Broken Bones", "Apply icepack to bruised areas"]),
            ]),
            opt("Spinal patient", vec![
                sec("Assessing possible spinal", &["Gently check the back of the patient's neck for any issues", "Run finger down the patient's spine checking for feeling/protrusions", "Gently squeeze the patient's limbs and ask if they have feeling"]),
                sec("Treatment", &["Apply a C collar as precaution when injury or no feeling is present", "Lift patient onto spinal board (if bystander present get them to help)", "Strap patient in", "Treat patient's other injuries"]),
            ]),
            opt("Blunt injuries (weaponry)", vec![
                sec("Assess", &["Assess if patient has a spinal injury (if so, switch to Spinal patient)", "Assess the location of the injuries", "Check for any soft spots to skull if head injury", "Check for any breakage of the skin"]),
                sec("Treatment", &["Clean any cuts with saline and bandage", "Check for broken bones - if broken, see Broken Bones", "Apply icepack to bruised areas"]),
            ]),
        ], vec![], vec![]),
        choice("concussion", "Concussion / Heart Attack / Stroke", "injury", "Which is it?", vec![
            opt("Concussion", vec![
                sec("Assessment", &["Check head for any injury (cuts or bruises)", "Check neck for any injury (spinal), apply C collar", "Check for breathing", "If conscious ask patient their symptoms"]),
                info("Symptoms", &["Symptoms can include nausea, confusion and headache"]),
                sec("Cognition", &["Ask the patient their name", "Ask what has happened to them", "Ask what they were going to do before injury"]),
                sec("Treatment", &["Administer paracetamol to patient if not allergic", "Avoid aspirin - this could cause further harm"]),
            ]),
            opt("Heart attack / stroke", vec![
                info("Heart attack symptoms", &["Pressure, tightness, pain or a squeezing sensation in chest/arms", "Nausea, indigestion, heartburn or abdominal pain", "Shortness of breath, cold sweat", "Fatigue, lightheadedness or dizziness"]),
                info("Stroke symptoms", &["Onset of weakness or numbness on one side of the body", "Speech difficulty or confusion", "Difficulty seeing in one or both eyes or sudden severe headache", "Onset of dizziness, trouble walking or loss of balance"]),
                sec("Treatment", &["Give blood thinners (aspirin/nitroglycerin)", "Transport to hospital as critical"]),
            ]),
        ], vec![], vec![]),
        page("vitals", "Vitals", "reaction", vec![col(vec![
            sec("Checking vitals", &["Check heartrate - take ECG off your back or from Ambo", "Check blood pressure - use blood pressure cuff via third eye"]),
            info("What the vitals mean", &["Flatlining - 1 bag of IV fluid, 1 shock of DEFIB, request MICA", "Vitals low - 1 bag of IV fluid, if still low 1mg adrenaline", "Vitals stable - start or continue treatment", "Vitals high - 1 bag of IV fluid"]),
            sec("If vitals do not improve", &["Do another head to toe", "Attempt to find other injuries that may be affecting the patient", "Treat other injuries if found and then recheck vitals"]),
            if_mica("Still no other injuries", &["Call MICA"]),
            no_mica("Still no other injuries", &["Transport to hospital"]),
        ])]),
        choice("unresponsive", "Unresponsiveness", "reaction", "What is the patient's state?", vec![
            opt("Low to no pulse - flatlining", vec![
                sec("What to do", &["Perform CPR", "Step 1: Give chest 30 compressions", "Step 2: Give rescue breaths"]),
                sec("If patient vitals begin to return to normal", &["Stop CPR and place into recovery position"]),
                sec("Vitals do not improve (and there is no metal shrapnel)", &["Grab Automated External Defibrillator (AED) from ambulance", "Shock patient once"]),
                if_mica("Vitals do not return to normal", &["Call for MICA to assist", "Repeat CPR"]),
                no_mica("Vitals do not return to normal", &["Repeat CPR or take patient to hospital as critical"]),
            ]),
            opt("Unconscious", vec![
                sec("Check vitals", &["If they are out of normal ranges proceed to administer saline", "If vitals remain high administer nitroglycerin (5-10mcg per minute)", "If vitals remain low administer 1mg of adrenaline"]),
                sec("Vitals remain unstable", &["Do another head to toe check to look for further injuries", "If no other injuries transport to hospital as critical"]),
                sec("Vitals stabilise but patient remains unconscious", &["Check for any other injuries - if yes treat", "If there are no other injuries transport to hospital via stretcher"]),
                sec("Vitals stabilise and patient wakes", &["Start treating further injuries", "Assess if patient requires pain management"]),
            ]),
        ], vec![], vec![]),
        page("pain", "Pain Management", "reaction", vec![
            col(vec![
                sec("Assess pain level", &["Make sure to give pain medication if necessary after treatment", "Ask the patient's pain level on a scale from 1-10", "Ask if the patient has had any drugs or alcohol in the last 48 hours", "Ask if the patient has any allergies"]),
                info("If patient is suspected of drug seeking", &["Patients will occasionally lie to get a high", "Give them a lower pain medication that you deem appropriate", "NEVER give the patient pain medication if they ask for it by name"]),
            ]),
            col(vec![
                info("Medications (pain level)", &["1-6: Nil (panadol or ibuprofen)", "6-7: Green whistle (oral)", "7-8: Fentanyl (nasal or IV)", "8-9: Morphine (IV)", "9-10: Ketamine (IV)"]),
                info("Ketamine", &["DO NOT GIVE KETAMINE UNLESS IT IS NECESSARY", "Ketamine is extremely addictive", "Use for transfer to/from vehicle or surgery only"]),
                info("Patients in custody", &["DO NOT give pain medication to patients in custody", "UNLESS approved by the arresting officer, or a dire emergency"]),
            ]),
        ]),
        page("codes", "RTO Codes", "reference", vec![col(vec![
            info("Remember RTO etiquette", &["Code 1 - On duty", "Code 2 - Arrived at hospital (no patients)", "Code 3 - Meal break", "Code 4 - Refueling", "Code 5 - Arrived at job/location", "Code 6 - Enroute to hospital (# of patients)", "Code 7 - Enroute to hospital (no patients)", "Code 8 - Off duty", "Code 9 - Duress! Urgent assistance needed", "Code 10 - Cardiac assistance required", "Code 11 - AFK/Busy", "Code 12 - MVA"]),
        ])]),
        page("radio", "Radio Calls", "reference", vec![col(vec![
            info("Most used", &["PRIORITY 1 TO CALL ID # / latest job (logging the job)", "CODE 5 (arrived at the job)", "CODE 5 (no patient present) patient has been taken/alphaed", "CODE 6 ENROUTE to (hospital name) # of patients", "CODE 7 ENROUTE to (hospital name) (no patients)"]),
        ])]),
        page("conversations", "Patient Conversations", "reference", vec![col(vec![
            sec("Talking to the patient", &["Hi, I'm [your name] from Victoria Ambulance", "Can you hear me?", "Can you open your eyes?", "What is your name?", "Can you squeeze my hand?", "Before I start can you tell me what hurts the most?", "Do you have any allergies?", "Had any alcohol or drugs in the last 48 hours?", "I'm going to check you from head to toe now"]),
            info("For fun", &["Have you heard of Darth Plagueis the Wise?", "Hey you, you're finally awake"]),
        ])]),
        Page {
            id: "vehicles", title: "Vehicles - Speeds and Limitations", group: "reference", icon: "", color: [0, 0, 0],
            kind: Kind::Tables(vec![
                Table { title: "Vehicle usage", head: vec!["Vehicle", "Certification", "Spinals", "Patients"], rows: vec![
                    vec!["Sprinter", "All Paramedics", "2", "2"],
                    vec!["Vito Van", "All Paramedics", "0", "0"],
                    vec!["Kluger", "Crew Supervisors/MICA - or difficult terrain calls", "1", "1"],
                    vec!["Landcruiser", "Off-Road Certified", "1", "1"],
                    vec!["RRU Bike", "RRU Certified Paramedics (3 sprinters minimum)", "0", "0"],
                    vec!["HEMS", "HEMS Certified Paramedics", "2", "2"],
                ] },
                Table { title: "Transport restrictions and speed limits", head: vec!["Transport", "Speed"], rows: vec![
                    vec!["Code Black", "Normal road rules"],
                    vec!["Priority 2", "Normal road rules"],
                    vec!["Priority 1", "100km/h rural/urban · 140km/h highways"],
                    vec!["Critical / Spinal", "80km/h rural/urban · 100km/h highways · avoid dirt roads if possible"],
                ] },
            ]),
        },
    ]
}

/// Each page's emoji and accent colour. Change the look of a page here.
/// The emoji is drawn from the matching picture in assets/emoji.
fn look(id: &str) -> (&'static str, [u8; 3]) {
    match id {
        "broken-bones" => ("1f9b4", [0xf2, 0xc1, 0x4e]),
        "drowning" => ("1f30a", [0x4f, 0xc3, 0xf7]),
        "stabbing" => ("1fa79", [0xef, 0x6a, 0x6a]),
        "drug-od" => ("1f48a", [0xc5, 0x8a, 0xf0]),
        "gunshot" => ("1f3af", [0xff, 0x9a, 0x6b]),
        "burns" => ("1f525", [0xff, 0x7a, 0x4d]),
        "blunt-force" => ("1f697", [0x8f, 0xa2, 0xf5]),
        "concussion" => ("1f9e0", [0xf5, 0x7f, 0xb0]),
        "vitals" => ("1f493", [0xff, 0x6b, 0x88]),
        "unresponsive" => ("1f634", [0xa5, 0x8c, 0xf0]),
        "pain" => ("1f489", [0x5f, 0xd0, 0xc0]),
        "codes" => ("1f4fb", [0xff, 0xd5, 0x4f]),
        "radio" => ("1f399", [0x4d, 0xd0, 0xe1]),
        "conversations" => ("1f4ac", [0x86, 0xd9, 0x8a]),
        "vehicles" => ("1f691", [0x6f, 0xb6, 0xff]),
        _ => ("1f4cb", [0xa2, 0xcb, 0xed]),
    }
}

pub struct Guide {
    /// Home first, then every page in reading order.
    pub pages: Vec<Page>,
}

impl Guide {
    pub fn page(&self, id: &str) -> Option<&Page> {
        self.pages.iter().find(|p| p.id == id)
    }
    pub fn order(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.pages.iter().map(|p| p.id)
    }
    pub fn pages_in<'a>(&'a self, group: &'a str) -> impl Iterator<Item = &'a Page> + 'a {
        self.pages.iter().filter(move |p| p.group == group)
    }
}

pub fn guide() -> &'static Guide {
    static GUIDE: OnceLock<Guide> = OnceLock::new();
    GUIDE.get_or_init(|| {
        let mut pages = vec![Page { id: "home", title: "Field Guide", group: "", icon: "1f3e0", color: [0xa2, 0xcb, 0xed], kind: Kind::Home }];
        for mut p in list() {
            let (icon, color) = look(p.id);
            p.icon = icon;
            p.color = color;
            pages.push(p);
        }
        Guide { pages }
    })
}

/// Is this section showing, given the MICA answer (None = not answered yet)?
pub fn shows_for(section: &Section, mica: Option<bool>) -> bool {
    match section.mica {
        None => true,
        Some(m) => mica == Some(m),
    }
}

/// The sections currently in play on a page, each with the prefix its step keys use.
/// For a choice page that is the intro, the chosen option (if any) and the outro.
pub fn sections_of(id: &str, option: Option<usize>) -> Vec<(&'static Section, String)> {
    let mut out = Vec::new();
    let Some(page) = guide().page(id) else { return out };
    let mut add = |prefix: String, sections: &'static [Section]| {
        for (si, s) in sections.iter().enumerate() {
            out.push((s, format!("{prefix}-{si}")));
        }
    };
    match &page.kind {
        Kind::Choice { options, intro, outro, .. } => {
            add("i".into(), intro);
            if let Some(o) = option.and_then(|o| options.get(o).map(|opt| (o, opt))) {
                add(format!("o{}", o.0), &o.1.sections);
            }
            add("x".into(), outro);
        }
        Kind::Columns(cols) => {
            for (ci, c) in cols.iter().enumerate() {
                add(ci.to_string(), &c.sections);
            }
        }
        _ => {}
    }
    out
}

/// Keys of the tick-off steps currently showing on a page, in reading order.
pub fn step_keys(id: &str, option: Option<usize>, mica: Option<bool>) -> Vec<String> {
    sections_of(id, option)
        .into_iter()
        .filter(|(s, _)| !s.info && shows_for(s, mica))
        .flat_map(|(s, prefix)| (0..s.steps.len()).map(move |i| format!("{prefix}-{i}")))
        .collect()
}

/// Does what's in play on this page include anything that depends on MICA?
pub fn asks_mica(id: &str, option: Option<usize>) -> bool {
    sections_of(id, option).iter().any(|(s, _)| s.mica.is_some())
}
