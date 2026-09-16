/// Dominant language of a transcript and the recognizer's confidence, like NLLanguageRecognizer on iPhone.
#[cfg(target_os = "macos")]
pub fn detect(text: &str) -> Option<(String, f64)> {
    use objc2_foundation::NSString;
    use objc2_natural_language::NLLanguageRecognizer;
    unsafe {
        let recognizer = NLLanguageRecognizer::new();
        recognizer.processString(&NSString::from_str(text));
        let hypotheses = recognizer.languageHypothesesWithMaximum(2);
        let mut best: Option<(String, f64)> = None;
        for key in hypotheses.allKeys().iter() {
            let Some(value) = hypotheses.objectForKey(&key) else { continue };
            let confidence = value.doubleValue();
            if best.as_ref().map(|b| confidence > b.1).unwrap_or(true) {
                best = Some((key.to_string(), confidence));
            }
        }
        best
    }
}

#[cfg(not(target_os = "macos"))]
pub fn detect(_text: &str) -> Option<(String, f64)> { None }
