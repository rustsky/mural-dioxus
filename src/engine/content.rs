// Generated from apps/ios/Core/Themes.swift and Core/Languages/*.swift.
use super::languages::{ConversationTheme, LanguageModule};

pub fn shared_themes() -> Vec<ConversationTheme> {
    vec![
        ConversationTheme::new("coffee", "A coffee?", "Something warm, please", "cup.and.saucer", "Everyday", "You work in a cosy café. Help the learner order, then chat naturally.", 0),
        ConversationTheme::new("weekend", "The weekend", "Tell me about yours", "sun.horizon", "Connection", "Ask about the learner’s weekend. Practise past events and follow their interests.", 1),
        ConversationTheme::new("walk", "A little walk", "Out into the fresh air", "tree", "Local life", "Take an imagined forest walk together. Talk about nature, weather and daily life.", 2),
        ConversationTheme::new("dinner", "Dinner plans", "Let’s make something", "fork.knife", "Everyday", "Plan dinner together. Ask about ingredients, preferences and the steps of cooking.", 3),
        ConversationTheme::new("introductions", "Nice to meet you", "Start somewhere small", "hand.wave", "Connection", "Meet the learner for the first time. Learn their interests through natural introductions.", 0),
        ConversationTheme::new("groceries", "At the market", "Find the good tomatoes", "basket", "Everyday", "Help the learner shop at a local food market. Practise quantities and questions.", 2),
        ConversationTheme::new("travel", "Next stop", "A ticket to somewhere", "tram", "Everyday", "Plan a train trip. Discuss routes and tickets without inventing real current schedules.", 1),
        ConversationTheme::new("home", "A place of your own", "Make yourself at home", "house", "Everyday", "Discuss a home, rooms, moving and what makes a place comfortable.", 3),
        ConversationTheme::new("friends", "New friends", "An invitation, maybe", "person.2", "Connection", "You are a friendly new acquaintance. Arrange something to do together.", 0),
        ConversationTheme::new("work", "Monday morning", "Around the office", "briefcase", "Everyday", "Chat as colleagues. Discuss work, meetings and a small problem to solve.", 1),
        ConversationTheme::new("weather", "Rain again?", "Whatever the weather", "cloud.rain", "Local life", "Talk about weather, clothing and outdoor plans. Do not claim today’s forecast without sources.", 1),
        ConversationTheme::new("cabin", "A weekend away", "A quieter kind of day", "mountain.2", "Local life", "Plan a weekend away: travel, food, walks and relaxing together.", 2),
        ConversationTheme::new("music", "On repeat", "What are you listening to?", "music.note", "Interests", "Ask about music the learner enjoys. Explore feelings, favourites and concerts.", 0),
        ConversationTheme::new("film", "One more episode", "Something worth watching", "film", "Interests", "Discuss films and series. Ask for opinions and avoid unwanted spoilers.", 1),
        ConversationTheme::new("books", "Between the pages", "A story that stayed", "book", "Interests", "Chat about books, characters, stories and why they matter to the learner.", 3),
        ConversationTheme::new("design", "Good things", "Made with a little care", "pencil.and.outline", "Interests", "Explore design, architecture and objects the learner loves. Ask for concrete opinions.", 0),
        ConversationTheme::new("technology", "What comes next", "Ideas, tools and tomorrow", "sparkles", "Interests", "Discuss technology and how it changes daily life. Delegate claims needing current facts.", 1),
        ConversationTheme::new("travelstories", "Somewhere else", "A place you remember", "globe.europe.africa", "Interests", "Exchange travel stories and dream destinations. Invite descriptions and comparisons.", 2),
        ConversationTheme::new("restaurant", "A table for two", "Stay for dessert", "wineglass", "Everyday", "Role-play a restaurant meal. Practise requests, preferences and polite problem-solving.", 0),
        ConversationTheme::new("neighbours", "Next door", "A familiar face", "building.2", "Connection", "Chat as neighbours. Discuss the neighbourhood and small requests for help.", 3),
        ConversationTheme::new("traditions", "Everyday customs", "Small customs, big stories", "flag", "Local life", "Explore everyday customs with nuance. Avoid treating a whole culture as alike.", 2),
        ConversationTheme::new("opinions", "What do you think?", "Room for another view", "quote.bubble", "Connection", "Choose an everyday dilemma. Invite reasons and gently explore another perspective.", 1),
        ConversationTheme::new("future", "A year from now", "Plans worth talking about", "paperplane", "Connection", "Talk about hopes and future plans. Explore possibilities and practical next steps.", 3),
        ConversationTheme::new("today", "The world today", "Something to talk about", "newspaper", "Interests", "Ask what current topic interests the learner, then delegate a source-backed lookup before discussing facts.", 0),
    ]
}

pub fn norwegian() -> LanguageModule {
    LanguageModule {
        id: "nb", name: "Norwegian", native_name: "Norsk", variety: "Bokmål", locale: "nb-NO",
        greeting: "Hei!", greeting_word: "hei",
        speech_guidance: "Use natural Eastern Norwegian pronunciation. Accept other Norwegian dialects without treating dialect differences as errors.",
        writing_guidance: "Use Norwegian Bokmål spelling and wording.",
        lemma_guidance: "Give nouns with their singular grammatical article and verbs in the infinitive, for example en tur and å gå. Accept valid gender variants.",
        teaching_focus: [
            "Greetings, introductions and short everyday chunks.",
            "Simple questions, noun gender and present-tense everyday exchanges.",
            "Connected stories, past tense, word order and familiar situations.",
            "Reasons and opinions, subordinate clauses and natural connectors.",
            "Nuanced discussion, idiomatic phrasing and register.",
            "Flexible advanced conversation with precise, natural Norwegian.",
        ],
        topic_placeholder: "Design, space, life in Norway…",
        lookup_unavailable_reply: "Jeg klarte ikke å sjekke det akkurat nå. Vi kan snakke om temaet generelt, hvis du vil.",
        theme_overrides: vec![
            ConversationTheme::new("groceries", "At the market", "Find the good tomatoes", "basket", "Everyday", "Help the learner shop at a Norwegian food market. Practise quantities and questions.", 2),
            ConversationTheme::new("travel", "Next stop", "A ticket to somewhere", "tram", "Everyday", "Plan a train trip in Norway. Discuss routes and tickets without inventing current schedules.", 1),
            ConversationTheme::new("weather", "Rain again?", "A very Norwegian chat", "cloud.rain", "Local life", "Talk about weather, clothing and outdoor plans in Norway. Verify current forecasts before claiming them.", 1),
            ConversationTheme::new("cabin", "Cabin weekend", "A quieter kind of day", "mountain.2", "Local life", "Plan a hytte weekend: travel, food, walks and relaxing together.", 2),
            ConversationTheme::new("traditions", "Life in Norway", "Small customs, big stories", "flag", "Local life", "Explore Norwegian everyday customs with nuance. Avoid treating all Norwegians as alike.", 2),
        ],
    }
}

pub fn spanish() -> LanguageModule {
    LanguageModule {
        id: "es", name: "Spanish", native_name: "Español", variety: "Spain", locale: "es-ES",
        greeting: "¡Hola!", greeting_word: "hola",
        speech_guidance: "Use clear Spanish from Spain, with a natural distinction between s and z/soft c, tú for friendly singular address and vosotros for informal plural address. Accept seseo, ustedes, voseo and other valid regional forms without marking them wrong. Do not imitate a regional caricature.",
        writing_guidance: "Use standard Spanish spelling, accents and opening question and exclamation marks.",
        lemma_guidance: "Give nouns with their singular grammatical article and verbs in the infinitive, for example la casa and hablar. Keep reflexive verbs such as llamarse distinct. Preserve accents and ñ.",
        teaching_focus: [
            "Greetings, introductions and short useful chunks such as me llamo and quiero.",
            "Everyday questions, gender and number agreement, present tense and useful ser/estar contrasts.",
            "Connected stories, past events, object pronouns and familiar situations.",
            "Reasons and opinions, contrasts between past tenses and common subjunctive contexts.",
            "Nuance, hypothetical situations, register and regional variation.",
            "Flexible advanced discussion with precise, idiomatic Spanish.",
        ],
        topic_placeholder: "Food, travel, music, life in Spain…",
        lookup_unavailable_reply: "No he podido comprobarlo ahora mismo. Si quieres, podemos hablar del tema en general.",
        theme_overrides: vec![
            ConversationTheme::new("coffee", "Un café", "Something warm, please", "cup.and.saucer", "Everyday", "Meet in a neighbourhood café in Spain. Order a drink and chat. Ask about the learner's interests.", 0),
            ConversationTheme::new("groceries", "En el mercado", "A little of everything", "basket", "Everyday", "Visit a local market in a Spanish-speaking community. Practise quantities, prices and polite questions. Respect regional food vocabulary.", 2),
            ConversationTheme::new("travel", "Next stop", "A ticket to somewhere", "tram", "Everyday", "Plan a trip in Spain. Discuss transport and tickets without inventing current schedules.", 1),
            ConversationTheme::new("cabin", "A weekend away", "Somewhere in the sunshine", "mountain.2", "Local life", "Plan an imagined weekend in a Spanish-speaking place. Choose a city, coast or countryside together and discuss practical plans.", 2),
            ConversationTheme::new("traditions", "La sobremesa", "Let the conversation linger", "fork.knife", "Local life", "Talk after a shared meal about daily routines, family and local customs. Compare experiences without treating Spanish-speaking cultures as uniform.", 2),
        ],
    }
}

pub fn english() -> LanguageModule {
    LanguageModule {
        id: "en", name: "English", native_name: "English", variety: "International", locale: "en",
        greeting: "Hi!", greeting_word: "hi",
        speech_guidance: "Use clear, broadly intelligible English with a consistent, natural pronunciation. Accept valid regional accents, vocabulary and grammar, including British and American forms. Do not treat an accent difference as an error or require imitation of a native accent. Correct pronunciation only when meaning is unclear and the audio supports the correction.",
        writing_guidance: "Use standard English spelling and punctuation. Keep one spelling convention within your own reply, but accept valid regional spelling and usage from the learner.",
        lemma_guidance: "Give countable nouns in the singular and verbs in the base form, for example a journey and go. Keep meaningful phrasal verbs such as look after together. Use a short, plain English definition as the stable sense rather than repeating the word itself.",
        teaching_focus: [
            "Greetings, introductions and useful everyday chunks such as I'd like and my name is.",
            "Everyday questions, present forms, articles and common countable and uncountable nouns.",
            "Connected stories, past events, future plans and familiar situations.",
            "Reasons and opinions, present perfect in context, conditionals and natural linking phrases.",
            "Nuance, idiomatic expressions, reported speech and appropriate register.",
            "Flexible advanced discussion with precise language, implication and tact.",
        ],
        topic_placeholder: "Travel, films, work, everyday life…",
        lookup_unavailable_reply: "I couldn't check that just now. We can talk about the topic more generally, if you like.",
        theme_overrides: vec![
            ConversationTheme::new("coffee", "A coffee?", "Something warm, please", "cup.and.saucer", "Everyday", "Meet in a neighbourhood café. Order a drink and chat in English. Follow the learner's interests and accept regional vocabulary.", 0),
            ConversationTheme::new("travel", "Next stop", "A ticket to somewhere", "tram", "Everyday", "Plan a trip using English. Let the learner choose the destination. Discuss transport and tickets without inventing current schedules.", 1),
            ConversationTheme::new("traditions", "Everyday customs", "Small customs, big stories", "flag", "Local life", "Compare everyday customs from places the learner knows. English is used across many cultures; avoid presenting one country's habits as universal.", 2),
        ],
    }
}

pub fn french() -> LanguageModule {
    LanguageModule {
        id: "fr", name: "French", native_name: "Français", variety: "France", locale: "fr-FR",
        greeting: "Salut !", greeting_word: "salut",
        speech_guidance: "Use clear, natural metropolitan French pronunciation. Use tu in a friendly conversation and vous when the situation calls for formality or plural address. Accept valid regional accents, vocabulary and grammar from across the French-speaking world. Do not treat regional variation, informal omission of ne or a non-native accent alone as an error. Do not imitate a regional caricature.",
        writing_guidance: "Use standard French spelling, accents, apostrophes and punctuation. Preserve accents on capital letters. Match the register to the situation and accept valid regional usage from the learner.",
        lemma_guidance: "Give nouns with a singular article that makes gender clear where possible and verbs in the infinitive, for example une maison, un ami and parler. Keep pronominal verbs such as se souvenir distinct. Preserve accents and meaningful elisions.",
        teaching_focus: [
            "Greetings, introductions and useful everyday chunks such as je m'appelle and je voudrais.",
            "Everyday questions, grammatical gender, present tense and common negation in conversation.",
            "Connected stories, passé composé and imparfait in context, future plans and familiar situations.",
            "Reasons and opinions, object pronouns, conditional requests and common subjunctive contexts.",
            "Nuance, hypothetical situations, register, idiomatic phrasing and regional variation.",
            "Flexible advanced discussion with precise, natural French and appropriate tone.",
        ],
        topic_placeholder: "Food, cinema, travel, life in France…",
        lookup_unavailable_reply: "Je n'ai pas pu vérifier ça pour le moment. On peut parler du sujet en général, si tu veux.",
        theme_overrides: vec![
            ConversationTheme::new("coffee", "Un café ?", "Something warm, please", "cup.and.saucer", "Everyday", "Meet in a neighbourhood café in France. Order a drink and chat. Use polite greetings with staff and a friendly register with the learner.", 0),
            ConversationTheme::new("groceries", "Au marché", "A little of everything", "basket", "Everyday", "Visit a local market in France. Practise quantities, prices and polite requests, then ask what the learner likes to cook.", 2),
            ConversationTheme::new("travel", "En route", "A ticket to somewhere", "tram", "Everyday", "Plan a trip in France. Discuss transport, directions and tickets without inventing current schedules.", 1),
            ConversationTheme::new("cabin", "A weekend away", "A change of scene", "mountain.2", "Local life", "Plan an imagined weekend in a French-speaking place. Choose a city, coast or countryside together and discuss practical plans.", 2),
            ConversationTheme::new("traditions", "À table", "Stay a little longer", "fork.knife", "Local life", "Talk over an imagined meal about daily routines and local customs. Compare the learner's experiences with life in France without treating French-speaking cultures as uniform.", 2),
        ],
    }
}

pub fn german() -> LanguageModule {
    LanguageModule {
        id: "de", name: "German", native_name: "Deutsch", variety: "Germany", locale: "de-DE",
        greeting: "Hallo!", greeting_word: "hallo",
        speech_guidance: "Use clear, natural Standard German as spoken in Germany. Use du for friendly conversation and Sie when the situation calls for formality. Accept valid Austrian, Swiss and other regional pronunciation, vocabulary and grammar. Do not treat a regional difference or a non-native accent alone as an error. Correct pronunciation only when supported by the audio, not a transcript alone.",
        writing_guidance: "Use standard German spelling, noun capitalization, umlauts and ß. Accept Swiss ss spellings and valid regional wording. Match the register to the situation.",
        lemma_guidance: "Give nouns with their singular article and verbs in the infinitive, for example das Haus, die Straße and sprechen. Preserve umlauts and ß. Keep separable verbs such as aufstehen and reflexive verbs such as sich erinnern together as dictionary entries, while quoting the learner's actual word order exactly.",
        teaching_focus: [
            "Greetings, introductions and useful everyday chunks such as ich heiße and ich möchte.",
            "Everyday questions, grammatical gender, present tense, verb-second word order and common accusative objects.",
            "Connected stories, conversational past tenses, dative uses, separable verbs and familiar situations.",
            "Reasons and opinions, subordinate-clause word order, relative clauses and polite Konjunktiv II requests.",
            "Nuance, hypothetical situations, passive voice, idiomatic phrasing and regional register.",
            "Flexible advanced discussion with precise, natural German and appropriate tone.",
        ],
        topic_placeholder: "Food, travel, music, life in Germany…",
        lookup_unavailable_reply: "Das konnte ich gerade nicht überprüfen. Wenn du möchtest, können wir allgemein über das Thema sprechen.",
        theme_overrides: vec![
            ConversationTheme::new("coffee", "Ein Kaffee?", "Something warm, please", "cup.and.saucer", "Everyday", "Meet in a neighbourhood café in Germany. Order a drink and chat. Use polite greetings with staff and follow the learner's interests.", 0),
            ConversationTheme::new("groceries", "Auf dem Markt", "A little of everything", "basket", "Everyday", "Shop at a weekly market in Germany. Practise quantities, prices and polite requests, accepting regional names for foods.", 2),
            ConversationTheme::new("travel", "Unterwegs", "A ticket to somewhere", "tram", "Everyday", "Plan a trip in Germany. Discuss transport, directions and tickets without inventing current schedules.", 1),
            ConversationTheme::new("cabin", "A weekend away", "A change of scene", "mountain.2", "Local life", "Plan an imagined weekend in a German-speaking place. Choose a city, coast or countryside together and discuss practical plans.", 2),
            ConversationTheme::new("traditions", "Feierabend", "After the working day", "flag", "Local life", "Talk about routines after work and local customs in Germany. Compare the learner's experiences without treating German-speaking cultures as uniform.", 2),
        ],
    }
}

pub fn italian() -> LanguageModule {
    LanguageModule {
        id: "it", name: "Italian", native_name: "Italiano", variety: "Italy", locale: "it-IT",
        greeting: "Ciao!", greeting_word: "ciao",
        speech_guidance: "Use clear, natural Standard Italian pronunciation. Use tu for friendly conversation and Lei when the situation calls for formality. Model vowel sounds, word stress and consonant length naturally. Accept valid regional accents and vocabulary without treating regional variation or a non-native accent alone as an error. Do not infer a pronunciation error from spelling alone.",
        writing_guidance: "Use standard Italian spelling, accents, apostrophes and punctuation. Preserve meaningful contrasts such as e and è. Match the register to the situation and accept valid regional usage.",
        lemma_guidance: "Give nouns with their singular article and verbs in the infinitive, for example la casa, lo studente and parlare. Preserve elisions and accents. Keep reflexive verbs such as chiamarsi and pronominal verbs such as farcela distinct.",
        teaching_focus: [
            "Greetings, introductions and useful everyday chunks such as mi chiamo and vorrei.",
            "Everyday questions, gender and number agreement, present tense and common prepositions.",
            "Connected stories, passato prossimo and imperfetto in context, future plans and familiar situations.",
            "Reasons and opinions, object pronouns, conditional requests and common congiuntivo contexts.",
            "Nuance, hypothetical situations, pronoun combinations, idiomatic phrasing and regional register.",
            "Flexible advanced discussion with precise, natural Italian and appropriate tone.",
        ],
        topic_placeholder: "Food, cinema, travel, life in Italy…",
        lookup_unavailable_reply: "Non sono riuscito a verificarlo adesso. Se vuoi, possiamo parlare dell'argomento in generale.",
        theme_overrides: vec![
            ConversationTheme::new("coffee", "Un caffè?", "Something warm, please", "cup.and.saucer", "Everyday", "Meet at a neighbourhood bar in Italy for a coffee. Order a drink, greet the staff politely and chat about the learner's day.", 0),
            ConversationTheme::new("groceries", "Al mercato", "A little of everything", "basket", "Everyday", "Visit a local market in Italy. Practise quantities, prices and polite requests, then ask what the learner likes to cook.", 2),
            ConversationTheme::new("travel", "In viaggio", "A ticket to somewhere", "tram", "Everyday", "Plan a trip in Italy. Discuss transport, directions and tickets without inventing current schedules.", 1),
            ConversationTheme::new("cabin", "A weekend away", "A change of scene", "mountain.2", "Local life", "Plan an imagined weekend in Italy. Choose a city, coast or countryside together and discuss practical plans.", 2),
            ConversationTheme::new("traditions", "La passeggiata", "An evening walk", "figure.walk", "Local life", "Take an imagined evening walk and discuss daily routines and local customs. Compare experiences without treating Italian communities as uniform.", 2),
        ],
    }
}

pub fn portuguese() -> LanguageModule {
    LanguageModule {
        id: "pt", name: "Portuguese", native_name: "Português", variety: "Brazil", locale: "pt-BR",
        greeting: "Olá!", greeting_word: "olá",
        speech_guidance: "Use clear, natural Brazilian Portuguese with broadly intelligible pronunciation and consistent Brazilian vocabulary. Use você in friendly conversation and formal address when appropriate. Accept valid uses of tu, regional Brazilian accents and grammar, and European, African and other Portuguese varieties without marking them wrong. Do not imitate a regional caricature or infer pronunciation errors from a transcript alone.",
        writing_guidance: "Use standard contemporary Brazilian Portuguese spelling, accents, ã, õ and ç. Prefer everyday Brazilian wording, including a gente and conversational pronoun placement when natural. Accept valid regional and European Portuguese usage from the learner.",
        lemma_guidance: "Give nouns with their singular article and verbs in the infinitive, for example a casa, o pão and falar. Preserve accents, nasal vowels and ç. Keep reflexive and pronominal verbs such as se lembrar distinct. Use a consistent Brazilian dictionary form without treating regional alternatives as errors.",
        teaching_focus: [
            "Greetings, introductions and useful everyday chunks such as meu nome é and eu gostaria de.",
            "Everyday questions, gender and number agreement, present tense, ser and estar, and você and a gente.",
            "Connected stories, pretérito perfeito and imperfeito in context, future plans and familiar situations.",
            "Reasons and opinions, object pronouns, polite requests and common subjunctive contexts.",
            "Nuance, future subjunctive, personal infinitive, hypothetical situations, idiomatic phrasing and regional register.",
            "Flexible advanced discussion with precise, natural Brazilian Portuguese and appropriate tone.",
        ],
        topic_placeholder: "Food, music, travel, life in Brazil…",
        lookup_unavailable_reply: "Não consegui verificar isso agora. Se quiser, podemos conversar sobre o assunto de forma geral.",
        theme_overrides: vec![
            ConversationTheme::new("coffee", "Um cafezinho?", "Something warm, please", "cup.and.saucer", "Everyday", "Meet at a neighbourhood café or padaria in Brazil. Order a drink and chat about the learner's day, using natural Brazilian vocabulary.", 0),
            ConversationTheme::new("groceries", "Na feira", "A little of everything", "basket", "Everyday", "Shop at a street market in Brazil. Practise quantities, prices and polite requests, respecting regional food names.", 2),
            ConversationTheme::new("travel", "Pé na estrada", "A ticket to somewhere", "tram", "Everyday", "Plan a trip in Brazil. Discuss transport, directions and tickets without inventing current schedules.", 1),
            ConversationTheme::new("cabin", "A weekend away", "A change of scene", "mountain.2", "Local life", "Plan an imagined weekend in Brazil. Choose a city, coast or countryside together and discuss practical plans.", 2),
            ConversationTheme::new("traditions", "Uma conversa à mesa", "Stay a little longer", "fork.knife", "Local life", "Talk over an imagined meal about routines and local customs in Brazil. Compare the learner's experiences without treating Brazilian or Portuguese-speaking cultures as uniform.", 2),
        ],
    }
}

pub fn mandarin() -> LanguageModule {
    LanguageModule {
        id: "zh", name: "Mandarin Chinese", native_name: "普通话", variety: "Mainland China", locale: "zh-CN",
        greeting: "你好！", greeting_word: "你好",
        speech_guidance: "Use clear, natural Standard Mandarin pronunciation. Treat tones, tone changes, retroflex and non-retroflex sounds, and distinctions between initials and finals as meaningful when they affect understanding. Accept valid regional accents and vocabulary without treating a regional difference or a non-native accent alone as an error. Do not imitate a regional caricature.",
        writing_guidance: "Use natural Simplified Chinese and standard modern punctuation. Prefer everyday Mainland usage while accepting valid regional wording and Traditional Chinese input. Keep Chinese text free of unnecessary spaces. The app displays pinyin separately; do not append pinyin or translations to ordinary spoken replies. Explain characters and tones briefly in Mandarin when asked.",
        lemma_guidance: "Give vocabulary lemmas in simplified characters only, with no pinyin or English in the lemma; the app supplies pronunciation help separately. Keep the exact observed form and quote, including Traditional Chinese or learner-written pinyin. Use dictionary forms and preserve meaningful chunks such as 洗澡 and 见面. Do not infer tone accuracy, pronunciation or spoken recall from typed pinyin or a transcript alone.",
        teaching_focus: [
            "Greetings, introductions and useful everyday chunks such as 我叫 and 我想要.",
            "Everyday questions, word order, measure words, numbers and common present-time exchanges.",
            "Connected stories, completed actions with 了, experiences with 过, and familiar situations.",
            "Reasons and opinions, comparisons, 把 and 被 constructions, and natural linking phrases.",
            "Nuance, aspect, conditionals, idiomatic phrasing, register and regional variation.",
            "Flexible advanced discussion with precise, natural Mandarin and appropriate tone.",
        ],
        topic_placeholder: "Food, travel, films, everyday life…",
        lookup_unavailable_reply: "我现在没法查证这件事。如果你愿意，我们可以先聊聊这个话题的一般情况。",
        theme_overrides: vec![
            ConversationTheme::new("coffee", "喝杯咖啡？", "Something warm, please", "cup.and.saucer", "Everyday", "在一家社区咖啡馆见面。用普通话点饮料并聊天，跟着学习者的兴趣展开对话。", 0),
            ConversationTheme::new("groceries", "去买菜", "Find something good", "basket", "Everyday", "在菜市场或超市买日常食材。练习数量、价格和礼貌的提问，尊重不同地区的食物词汇。", 2),
            ConversationTheme::new("travel", "下一站", "A ticket to somewhere", "tram", "Everyday", "用普通话计划一次旅行。讨论交通、方向和买票，不要编造当前的时刻表。", 1),
            ConversationTheme::new("cabin", "周末出游", "A change of scene", "mountain.2", "Local life", "一起设想一个周末旅行，选择城市、海边或乡村，讨论实际安排和喜欢做的事情。", 2),
            ConversationTheme::new("traditions", "日常习俗", "Small customs, big stories", "flag", "Local life", "用普通话聊日常习俗和节日。比较学习者熟悉的地方，避免把任何一种习惯说成所有人的共同体验。", 2),
        ],
    }
}

pub fn all() -> Vec<LanguageModule> {
    vec![norwegian(), spanish(), english(), french(), german(), italian(), portuguese(), mandarin()]
}
