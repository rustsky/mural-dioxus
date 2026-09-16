use std::collections::BTreeSet;

use serde_json::Value;

use super::activity::{Action, ConversationActivity};
use super::languages;
use super::learning::LearningEngine;
use super::models::*;
use super::pinyin;
use super::provider::{ProviderFailure, ProviderFailureKind};
use super::teaching;
use super::time::{Date, REFERENCE_UNIX_OFFSET};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cross-platform");

fn fixture_file(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture")
}

fn fixture(day: f64, theme: &str, supported: bool, kind: EvidenceKind) -> SessionRecord {
    let date = Date(1_780_000_000.0 + day * 86_400.0 - REFERENCE_UNIX_OFFSET);
    let mut s = SessionRecord::new("nb", Some(theme.into()), None);
    s.started_at = date;
    let mut f = Fragment::new(Speaker::User, "Jeg gikk i skogen.", 1000, 2000);
    f.received_at = date;
    f.meaning_visible = supported;
    s.append(f);
    let p = s.passages()[0].clone();
    s.assessments = vec![Assessment {
        passage_id: p.id.clone(), revision_key: p.revision_key(), outcome: Outcome::Success, suggested_level: 2,
        next_goal: "Fortell mer.".into(), capability: "Describes a past outing".into(),
        words: vec![WordProposal {
            lemma: "å gå".into(), meaning: "to go".into(), form: "gikk".into(), kind, confidence: 0.95,
            source_ids: p.fragments.iter().map(|f| f.id.clone()).collect(), quote: "Jeg gikk i skogen.".into(), language: "nb".into(),
        }],
        created_at: date, context: theme.into(),
    }];
    s
}

fn basic() -> SessionRecord { fixture(0.0, "walk", false, EvidenceKind::Independent) }

fn frag(id: &str, speaker: Speaker, text: &str, start: i64, end: i64) -> Fragment {
    let mut f = Fragment::new(speaker, text, start, end);
    f.id = id.into();
    f
}

#[test]
fn redirect_decisions_match_shared_cases() {
    let root: Value = serde_json::from_slice(&fixture_file("redirect-cases.json")).unwrap();
    let cases = root["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for item in cases {
        let language = languages::module(item["language"].as_str().unwrap()).unwrap();
        let got = teaching::should_redirect_speech(language, item["detected"].as_str().unwrap(), item["confidence"].as_f64().unwrap());
        assert_eq!(got, item["redirect"].as_bool().unwrap(), "{item}");
    }
}

fn normalized(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(map.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), normalized(v))).collect()),
        Value::Array(items) => Value::Array(items.iter().map(normalized).collect()),
        Value::Number(n) => serde_json::json!(n.as_f64().unwrap()),
        other => other.clone(),
    }
}

fn field_paths(value: &Value, prefix: &str, paths: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => for (k, child) in map {
            if child.is_null() { continue; }
            let path = if prefix.ends_with(".translations") { format!("{prefix}.{{}}") } else { format!("{prefix}.{k}") };
            paths.insert(path.clone());
            field_paths(child, &path, paths);
        },
        Value::Array(items) => for item in items { field_paths(item, &format!("{prefix}[]"), paths) },
        _ => {}
    }
}

#[test]
fn reencoded_archive_keeps_every_field_of_shared_fixture() {
    let data = fixture_file("archive.json");
    let archive = Archive::decode(&data).unwrap();
    let reencoded = archive.encoded().unwrap();
    let (a, b): (Value, Value) = (serde_json::from_slice(&data).unwrap(), serde_json::from_slice(&reencoded).unwrap());
    let (mut pa, mut pb) = (BTreeSet::new(), BTreeSet::new());
    field_paths(&a, "", &mut pa);
    field_paths(&b, "", &mut pb);
    assert_eq!(pa, pb);
    assert_eq!(normalized(&a), normalized(&b));
    let again = Archive::decode(&reencoded).unwrap();
    let texts = |a: &Archive| a.sessions.iter().map(|s| s.passages().iter().map(|p| p.text()).collect::<Vec<_>>()).collect::<Vec<_>>();
    assert_eq!(texts(&again), texts(&archive));
}

#[test]
fn transcript_passages_match_shared_fixture() {
    let archive = Archive::decode(&fixture_file("archive.json")).unwrap();
    let expected: Value = serde_json::from_slice(&fixture_file("archive-expected.json")).unwrap();
    let passages = expected["passages"].as_object().unwrap();
    assert_eq!(passages.len(), archive.sessions.len());
    for session in &archive.sessions {
        let want = passages[&session.id].as_array().unwrap();
        let got = session.passages();
        assert_eq!(want.len(), got.len(), "{}", session.id);
        for (item, passage) in want.iter().zip(got.iter()) {
            assert_eq!(item["speaker"].as_str().unwrap(), passage.speaker.raw());
            assert_eq!(item["text"].as_str().unwrap(), passage.text());
            let ids: Vec<&str> = item["fragmentIDs"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
            assert_eq!(ids, passage.fragments.iter().map(|f| f.id.as_str()).collect::<Vec<_>>());
        }
    }
}

#[test]
fn learner_projection_matches_shared_fixture() {
    let archive = Archive::decode(&fixture_file("archive.json")).unwrap();
    let fixture: Value = serde_json::from_slice(&fixture_file("archive-expected.json")).unwrap();
    let learner = &fixture["learner"];
    let state = LearningEngine::project(&archive.sessions, fixture["languageID"].as_str().unwrap(),
        &archive.preferences.hidden_words, Date(fixture["now"].as_f64().unwrap()));
    assert_eq!(learner["challenge"].as_i64().unwrap(), state.challenge);
    assert_eq!(learner["observationCount"].as_u64().unwrap() as usize, state.observation_count);
    assert_eq!(learner["nextGoal"].as_str().unwrap(), state.next_goal);
    let caps: Vec<&str> = learner["capabilities"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(caps, state.capabilities);
    let want = learner["words"].as_array().unwrap();
    let mut words = state.words.clone();
    words.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(want.iter().map(|w| w["id"].as_str().unwrap()).collect::<Vec<_>>(), words.iter().map(|w| w.id.as_str()).collect::<Vec<_>>());
    for (item, word) in want.iter().zip(words.iter()) {
        assert_eq!(item["lemma"].as_str().unwrap(), word.lemma);
        assert_eq!(item["meaning"].as_str().unwrap(), word.meaning);
        assert_eq!(item["form"].as_str().unwrap(), word.form);
        assert_eq!(item["example"].as_str().unwrap(), word.example);
        assert_eq!(item["bars"].as_u64().unwrap() as usize, word.bars);
        assert_eq!(item["understandingCount"].as_u64().unwrap() as usize, word.understanding_count);
        assert_eq!(item["independentCount"].as_u64().unwrap() as usize, word.independent_count);
        assert_eq!(item["lastSeen"].as_f64().unwrap(), word.last_seen.0);
        assert_eq!(item["dueAt"].as_f64().unwrap(), word.due_at.0);
    }
}

#[test]
fn duplicate_provider_events_do_not_change_transcript() {
    let mut s = SessionRecord::new("nb", None, None);
    let f = frag("same", Speaker::User, "Hei", 0, 100);
    s.append(f.clone());
    s.append(f);
    assert_eq!(s.fragments.len(), 1);
}

#[test]
fn concatenation_rules() {
    let f = [frag("a", Speaker::Assistant, "Hva", 0, 100), frag("b", Speaker::Assistant, " gjorde du?", 100, 400)];
    assert_eq!(passages(&f)[0].text(), "Hva gjorde du?");
    let f = [frag("a", Speaker::Assistant, "It is easy.", 0, 100), frag("b", Speaker::Assistant, "Now you?", 400, 700)];
    assert_eq!(passages(&f)[0].text(), "It is easy. Now you?");
    let f = [frag("c", Speaker::Assistant, "Hei", 0, 100), frag("d", Speaker::Assistant, "!", 100, 150)];
    assert_eq!(passages(&f)[0].text(), "Hei!");
    assert_eq!(Passage::join(["我", "喜欢", "咖啡。", "你呢？"]), "我喜欢咖啡。你呢？");
    assert_eq!(Passage::join(["“", "Hola", "!”"]), "“Hola!”");
    assert_eq!(Passage::join(["Hola\u{00A0}", "mundo"]), "Hola\u{00A0}mundo");
    assert_eq!(Passage::join(["", "Hello.", "", "Again."]), "Hello. Again.");
}

#[test]
fn late_fragments_invalidate_evidence_and_speakers_stay_separate() {
    let mut s = basic();
    s.append(frag("late", Speaker::User, " kanskje", 2100, 2500));
    assert!(s.assessments.is_empty());
    assert_eq!(s.passages().len(), 1);
    let f = [frag("1", Speaker::Assistant, "Hei", 0, 500), frag("2", Speaker::User, "Hallo", 100, 400), frag("3", Speaker::Assistant, "!", 500, 600)];
    let p = passages(&f);
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].text(), "Hei!");
    assert_eq!(p[1].text(), "Hallo");
}

#[test]
fn supported_production_is_never_independent() {
    let s = fixture(0.0, "walk", true, EvidenceKind::Independent);
    assert_eq!(LearningEngine::validate(&s.assessments[0], &s).unwrap().words[0].kind, EvidenceKind::Assisted);
    assert_eq!(LearningEngine::project(&[s], "nb", &[], Date::now()).words[0].independent_count, 0);

    let mut s = basic();
    s.fragments.insert(0, Fragment::new(Speaker::Assistant, "Du gikk en tur?", 0, 500));
    assert_eq!(LearningEngine::validate(&s.assessments[0], &s).unwrap().words[0].kind, EvidenceKind::Assisted);

    let mut s = basic();
    s.fragments[0].typed = true;
    assert_eq!(LearningEngine::validate(&s.assessments[0], &s).unwrap().words[0].kind, EvidenceKind::Assisted);
}

#[test]
fn fabricated_or_foreign_evidence_is_rejected() {
    let mut s = basic();
    s.assessments[0].words[0].language = "en".into();
    assert!(LearningEngine::validate(&s.assessments[0], &s).unwrap().words.is_empty());
    let mut s = basic();
    s.assessments[0].words[0].source_ids = vec!["invented".into()];
    assert!(LearningEngine::validate(&s.assessments[0], &s).unwrap().words.is_empty());
    let mut s = basic();
    s.assessments[0].words[0].quote = "Jeg kan fly.".into();
    assert!(LearningEngine::validate(&s.assessments[0], &s).unwrap().words.is_empty());
}

#[test]
fn duplicates_never_double_credit() {
    let mut s = basic();
    let dup = s.assessments[0].clone();
    s.assessments.push(dup);
    let p = LearningEngine::project(&[s.clone()], "nb", &[], s.started_at);
    assert_eq!(p.words[0].independent_count, 1);
    assert_eq!(p.observation_count, 1);
    let mut s = basic();
    let w = s.assessments[0].words[0].clone();
    s.assessments[0].words.push(w);
    assert_eq!(LearningEngine::project(&[s.clone()], "nb", &[], s.started_at).words[0].independent_count, 1);
}

#[test]
fn steady_requires_spacing_and_contexts_and_fades() {
    let first = basic();
    let second = fixture(2.0, "walk", false, EvidenceKind::Independent);
    let third = fixture(8.0, "dinner", false, EvidenceKind::Independent);
    let all = [first.clone(), second.clone(), third.clone()];
    assert_eq!(LearningEngine::project(&all, "nb", &[], third.started_at).words[0].bars, 3);
    let same = [first.clone(), second.clone(), fixture(8.0, "walk", false, EvidenceKind::Independent)];
    assert_eq!(LearningEngine::project(&same, "nb", &[], third.started_at).words[0].bars, 2);
    let faded = LearningEngine::project(&all, "nb", &[], third.started_at.adding(30.0 * 86_400.0));
    assert_eq!(faded.words[0].bars, 2);
    let lapse = fixture(9.0, "walk", false, EvidenceKind::Lapse);
    let mut with_lapse = all.to_vec();
    with_lapse.push(lapse.clone());
    assert_eq!(LearningEngine::project(&with_lapse, "nb", &[], lapse.started_at).words[0].bars, 1);
}

#[test]
fn corrections_revoke_evidence_and_only_affected_translations() {
    let mut s = basic();
    let id = s.fragments[0].id.clone();
    s.translations.insert(format!("English::{id}:0"), "I went into the forest.".into());
    s.translations.insert("English::other:0".into(), "Unrelated meaning".into());
    s.correct_fragment(&id, "I went for a walk.");
    assert!(s.assessments.is_empty());
    assert!(!s.translations.contains_key(&format!("English::{id}:0")));
    assert_eq!(s.translations.get("English::other:0").map(String::as_str), Some("Unrelated meaning"));
}

#[test]
fn archive_round_trip_guards() {
    let mut archive = Archive::default();
    archive.sessions = vec![basic()];
    assert_eq!(Archive::decode(&archive.encoded().unwrap()).unwrap().sessions.len(), 1);
    let mut bad = archive.clone();
    bad.schema_version = 99;
    assert_eq!(Archive::decode(&bad.encoded().unwrap()), Err(ArchiveError::UnsupportedVersion));
    let mut dup = archive.clone();
    dup.sessions.push(dup.sessions[0].clone());
    assert!(Archive::decode(&dup.encoded().unwrap()).is_err());
    let mut big = archive.clone();
    big.sessions[0].voice_seconds = 1e308;
    assert!(Archive::decode(&big.encoded().unwrap()).is_err());
    let mut neg = archive.clone();
    neg.sessions[0].input_tokens = -1;
    assert!(Archive::decode(&neg.encoded().unwrap()).is_err());
}

#[test]
fn import_merge_keeps_local_preferences_and_validates_evidence() {
    let mut original = Archive::default();
    original.preferences.meaning_language = "Spanish".into();
    original.preferences.ai_consent_version = Some(1);
    let mut incoming = Archive::default();
    let mut invalid = basic();
    invalid.assessments[0].revision_key = "changed".into();
    incoming.sessions = vec![invalid];
    let merged = original.merging(&incoming).unwrap();
    assert_eq!(merged.preferences.meaning_language, "Spanish");
    assert_eq!(merged.preferences.ai_consent_version, Some(1));
    assert!(merged.sessions[0].assessments.is_empty());
}

#[test]
fn version_one_archives_migrate_to_norwegian() {
    let v1 = serde_json::json!({
        "schemaVersion": 1,
        "preferences": {"meaningVisible": true, "meaningLanguage": "English", "sessionMinutes": 15, "hiddenWords": ["hei|hello"], "interests": "", "hasOnboarded": true},
        "sessions": [{"id": "A", "startedAt": 0.0, "title": "t", "fragments": [], "assessments": [], "translations": {}, "topics": [], "voiceSeconds": 0.0, "usageFinal": false, "inputTokens": 0, "outputTokens": 0, "searchCalls": 0}]
    });
    let archive = Archive::decode(&serde_json::to_vec(&v1).unwrap()).unwrap();
    assert_eq!(archive.preferences.learning_language_id, "nb");
    assert_eq!(archive.preferences.hidden_words, vec!["nb|hei|hello".to_string()]);
    assert_eq!(archive.sessions[0].language_id, "nb");
}

#[test]
fn source_links_reject_non_https_and_credentials() {
    let link = |u: &str| SourceLink { title: "x".into(), url: u.into() };
    assert!(link("javascript:alert(1)").safe_url().is_none());
    assert!(link("https://user@example.com/page").safe_url().is_none());
    assert!(link("https://www.nrk.no/").safe_url().is_some());
}

#[test]
fn languages_have_distinct_themes() {
    assert_eq!(languages::all().len(), 8);
    for m in languages::all() {
        let ids: BTreeSet<String> = m.themes().into_iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 24, "{}", m.id);
    }
    assert_eq!(languages::module("es").unwrap().themes()[0].title, "Un café");
}

#[test]
fn provider_failures_keep_only_safe_details() {
    let f = ProviderFailure::new(429, br#"{"error":{"code":"insufficient_quota","message":"private"}}"#, Some("req_support_fixture"));
    assert_eq!(f.kind(), ProviderFailureKind::Quota);
    assert!(f.to_string().contains("req_support_fixture"));
    assert!(!f.to_string().contains("private"));
    assert_eq!(ProviderFailure::new(429, b"{}", Some("bad ref!")).reference, None);
    assert_eq!(ProviderFailureKind::classify(503, None), ProviderFailureKind::Unavailable);
}

#[test]
fn activity_checks_in_then_warns_then_ends() {
    let mut a = ConversationActivity::new(0.0);
    assert_eq!(a.tick(10.0, false, false), Action::Wait);
    assert_eq!(a.tick(15.0, false, false), Action::CheckIn);
    assert_eq!(a.tick(26.0, false, false), Action::Warning(4));
    assert_eq!(a.tick(30.0, false, false), Action::End);
}

#[cfg(target_os = "macos")]
#[test]
fn pinyin_uses_word_readings() {
    let reading = pinyin::reading("我喜欢旅行。").unwrap();
    assert!(reading.contains("lǚ"), "{reading}");
    let joined: String = pinyin::tokens("你好，世界").into_iter().map(|t| t.text).collect();
    assert_eq!(joined, "你好，世界");
    assert!(pinyin::reading("Hello").is_none());
}
