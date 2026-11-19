//! Translations: English, French, Spanish, German, Italian.

use std::cell::Cell;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En = 0,
    Fr = 1,
    Es = 2,
    De = 3,
    It = 4,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::En, Lang::Fr, Lang::Es, Lang::De, Lang::It];

    pub fn code(self) -> &'static str {
        ["en", "fr", "es", "de", "it"][self as usize]
    }

    pub fn native_name(self) -> &'static str {
        ["English", "Français", "Español", "Deutsch", "Italiano"][self as usize]
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        let c = code.to_lowercase();
        Lang::ALL.into_iter().find(|l| c.starts_with(l.code()))
    }

    /// First supported language in the macOS preferences.
    pub fn system() -> Lang {
        #[cfg(target_os = "macos")]
        {
            let langs = objc2_foundation::NSLocale::preferredLanguages();
            for l in langs.iter() {
                if let Some(lang) = Lang::from_code(&l.to_string()) {
                    return lang;
                }
            }
        }
        std::env::var("LANG")
            .ok()
            .and_then(|l| Lang::from_code(&l))
            .unwrap_or(Lang::En)
    }
}

thread_local! {
    static LANG: Cell<Lang> = const { Cell::new(Lang::En) };
}

pub fn set_lang(lang: Lang) {
    LANG.with(|l| l.set(lang));
}

pub fn lang() -> Lang {
    LANG.with(|l| l.get())
}

pub fn t(key: T) -> &'static str {
    key.get(lang())
}

macro_rules! strings {
    ($($key:ident => [$en:expr, $fr:expr, $es:expr, $de:expr, $it:expr $(,)?]),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum T { $($key),* }
        impl T {
            pub fn get(self, lang: Lang) -> &'static str {
                match self { $(T::$key => [$en, $fr, $es, $de, $it][lang as usize]),* }
            }
        }
    };
}

strings! {
    // ---- navigation ----------------------------------------------------------------
    TabControls => ["Controls", "Contrôle", "Control", "Steuerung", "Controlli"],
    TabEqualizer => ["Equalizer", "Égaliseur", "Ecualizador", "Equalizer", "Equalizzatore"],
    TabSettings => ["Settings", "Réglages", "Ajustes", "Einstellungen", "Impostazioni"],
    Unofficial => ["Unofficial", "Non officielle", "No oficial", "Inoffiziell", "Non ufficiale"],

    // ---- connection ----------------------------------------------------------------
    Connected => ["Connected", "Connecté", "Conectado", "Verbunden", "Connesso"],
    NotConnected => ["Not connected", "Non connecté", "No conectado", "Nicht verbunden", "Non connesso"],
    PhaseWaiting => ["Waiting for your headphones…", "En attente de votre casque…", "Esperando tus auriculares…", "Warte auf deinen Kopfhörer…", "In attesa delle cuffie…"],
    PhaseConnecting => ["Connecting…", "Connexion…", "Conectando…", "Verbinden…", "Connessione…"],
    PhaseLoading => ["Reading your settings…", "Lecture de vos réglages…", "Leyendo tus ajustes…", "Einstellungen werden gelesen…", "Lettura delle impostazioni…"],
    PhaseBusy => ["Controls in use by another app", "Commandes utilisées par une autre app", "Controles en uso por otra app", "Steuerung von anderer App belegt", "Controlli in uso da un'altra app"],
    PhaseFailed => ["Connection failed", "Échec de la connexion", "Error de conexión", "Verbindung fehlgeschlagen", "Connessione non riuscita"],
    HintWaiting => [
        "Turn your headphones on and connect them to this Mac as an audio device (System Settings > Bluetooth). The app connects as soon as they are there.",
        "Allumez votre casque et connectez-le à ce Mac comme appareil audio (Réglages Système > Bluetooth). L'app se connecte dès qu'il est là.",
        "Enciende tus auriculares y conéctalos a este Mac como dispositivo de audio (Ajustes del Sistema > Bluetooth). La app se conecta en cuanto estén.",
        "Schalte den Kopfhörer ein und verbinde ihn als Audiogerät mit diesem Mac (Systemeinstellungen > Bluetooth). Die App verbindet sich, sobald er da ist.",
        "Accendi le cuffie e collegale a questo Mac come dispositivo audio (Impostazioni di Sistema > Bluetooth). L'app si collega appena sono disponibili.",
    ],
    HintBusy => [
        "Only one app at a time can use your headphones' controls. Close the soundcore app on your phone (or turn its Bluetooth off for a moment): the connection resumes on its own.",
        "Une seule app à la fois peut utiliser les commandes du casque. Fermez l'app soundcore sur votre téléphone (ou coupez son Bluetooth un instant) : la connexion reprend toute seule.",
        "Solo una app a la vez puede usar los controles de los auriculares. Cierra la app soundcore del teléfono (o apaga su Bluetooth un momento): la conexión se reanuda sola.",
        "Nur eine App gleichzeitig kann den Kopfhörer steuern. Schließe die soundcore App auf deinem Handy (oder schalte dort kurz Bluetooth aus): Die Verbindung wird automatisch fortgesetzt.",
        "Una sola app alla volta può usare i controlli delle cuffie. Chiudi l'app soundcore sul telefono (o spegni per un attimo il Bluetooth): la connessione riprende da sola.",
    ],
    Retry => ["Try again", "Réessayer", "Reintentar", "Erneut versuchen", "Riprova"],

    // ---- connect screen ------------------------------------------------------------
    Tagline => [
        "The companion app for your soundcore headphones, on Mac",
        "L'app compagnon de votre casque soundcore, sur Mac",
        "La app complementaria de tus auriculares soundcore, en Mac",
        "Die Begleit-App für deinen soundcore Kopfhörer, auf dem Mac",
        "L'app compagna delle tue cuffie soundcore, su Mac",
    ],
    MyDevices => ["My devices", "Mes appareils", "Mis dispositivos", "Meine Geräte", "I miei dispositivi"],
    Connect => ["Connect", "Connecter", "Conectar", "Verbinden", "Connetti"],
    ShowAllPaired => ["Show all paired devices", "Afficher tous les appareils jumelés", "Mostrar todos los dispositivos", "Alle gekoppelten Geräte zeigen", "Mostra tutti i dispositivi"],
    ShowSoundcoreOnly => ["Show soundcore devices only", "Afficher seulement les appareils soundcore", "Mostrar solo dispositivos soundcore", "Nur soundcore Geräte zeigen", "Mostra solo dispositivi soundcore"],
    NoDevices => [
        "No soundcore headphones paired with this Mac yet. Put your headphones in pairing mode, then pair them in System Settings > Bluetooth.",
        "Aucun casque soundcore n'est encore jumelé avec ce Mac. Mettez votre casque en mode appairage, puis jumelez-le dans Réglages Système > Bluetooth.",
        "Aún no hay auriculares soundcore emparejados con este Mac. Pon los auriculares en modo de emparejamiento y emparéjalos en Ajustes del Sistema > Bluetooth.",
        "Noch kein soundcore Kopfhörer mit diesem Mac gekoppelt. Versetze den Kopfhörer in den Kopplungsmodus und kopple ihn unter Systemeinstellungen > Bluetooth.",
        "Nessuna cuffia soundcore abbinata a questo Mac. Metti le cuffie in modalità di abbinamento e abbinale in Impostazioni di Sistema > Bluetooth.",
    ],
    PairNew => ["Pair new headphones", "Jumeler un nouveau casque", "Emparejar auriculares", "Neuen Kopfhörer koppeln", "Abbina nuove cuffie"],
    InUse => ["In use", "Utilisé", "En uso", "Aktiv", "In uso"],
    Switch => ["Switch", "Choisir", "Cambiar", "Wechseln", "Passa"],
    Disconnect => ["Disconnect", "Déconnecter", "Desconectar", "Trennen", "Disconnetti"],

    // ---- controls page -------------------------------------------------------------
    Battery => ["Battery", "Batterie", "Batería", "Akku", "Batteria"],
    Charging => ["Charging", "En charge", "Cargando", "Lädt", "In carica"],
    Firmware => ["Firmware", "Firmware", "Firmware", "Firmware", "Firmware"],
    NoiseControl => ["Noise control", "Contrôle du bruit", "Control de ruido", "Geräuschkontrolle", "Controllo del rumore"],
    ModeAnc => ["Noise cancelling", "Réduction de bruit", "Cancelación de ruido", "Geräuschunterdrückung", "Cancellazione del rumore"],
    ModeNormal => ["Normal", "Normal", "Normal", "Normal", "Normale"],
    ModeTransparency => ["Transparency", "Transparence", "Transparencia", "Transparenz", "Trasparenza"],
    DescAnc => [
        "Hybrid noise cancelling blocks out engines, chatter and hum.",
        "La réduction de bruit hybride atténue moteurs, conversations et bourdonnements.",
        "La cancelación híbrida bloquea motores, conversaciones y zumbidos.",
        "Die hybride Geräuschunterdrückung blendet Motoren, Gespräche und Brummen aus.",
        "La cancellazione ibrida blocca motori, chiacchiere e ronzii.",
    ],
    DescNormal => [
        "No processing: your music, as it is.",
        "Aucun traitement : votre musique, telle quelle.",
        "Sin procesamiento: tu música, tal cual.",
        "Keine Verarbeitung: deine Musik, wie sie ist.",
        "Nessuna elaborazione: la tua musica, così com'è.",
    ],
    DescTransparency => [
        "Lets the sound around you in, so you can hear voices and announcements.",
        "Laisse passer les sons autour de vous, pour entendre les voix et les annonces.",
        "Deja pasar el sonido de tu alrededor para oír voces y avisos.",
        "Lässt Umgebungsgeräusche herein, damit du Stimmen und Durchsagen hörst.",
        "Lascia entrare i suoni intorno a te, per sentire voci e annunci.",
    ],
    OnYourHeadphones => ["On your headphones", "Sur votre casque", "En tus auriculares", "Am Kopfhörer", "Sulle cuffie"],
    TipModeButton => [
        "Press the mode button to cycle Noise cancelling, Normal and Transparency; the app follows.",
        "Appuyez sur le bouton de mode pour passer de Réduction de bruit à Normal puis Transparence ; l'app suit.",
        "Pulsa el botón de modo para pasar por Cancelación, Normal y Transparencia; la app lo refleja.",
        "Drücke die Modustaste, um zwischen Geräuschunterdrückung, Normal und Transparenz zu wechseln; die App zieht mit.",
        "Premi il tasto modalità per passare tra Cancellazione, Normale e Trasparenza; l'app si aggiorna.",
    ],
    TipVoiceAssistant => [
        "Hold the mode button for 2 seconds to call your voice assistant.",
        "Maintenez le bouton de mode 2 secondes pour appeler l'assistant vocal.",
        "Mantén pulsado el botón de modo 2 segundos para llamar al asistente de voz.",
        "Halte die Modustaste 2 Sekunden gedrückt, um den Sprachassistenten zu rufen.",
        "Tieni premuto il tasto modalità per 2 secondi per l'assistente vocale.",
    ],
    TipLed => [
        "Light: red when charging, flashing red when the battery is low, steady blue once connected.",
        "Voyant : rouge en charge, rouge clignotant si la batterie est faible, bleu fixe une fois connecté.",
        "Luz: roja al cargar, roja intermitente con batería baja, azul fija al estar conectado.",
        "Leuchte: rot beim Laden, rot blinkend bei schwachem Akku, dauerhaft blau, wenn verbunden.",
        "Spia: rossa in carica, rossa lampeggiante con batteria scarica, blu fissa quando connesse.",
    ],

    // ---- equalizer page ------------------------------------------------------------
    EqSubtitle => [
        "22 presets tuned for soundcore, or your own curve.",
        "22 préréglages taillés pour soundcore, ou votre propre courbe.",
        "22 ajustes pensados para soundcore, o tu propia curva.",
        "22 Voreinstellungen für soundcore oder deine eigene Kurve.",
        "22 preset pensati per soundcore, o la tua curva.",
    ],
    Presets => ["Presets", "Préréglages", "Ajustes", "Voreinstellungen", "Preset"],
    Custom => ["Custom", "Personnalisé", "Personalizado", "Benutzerdefiniert", "Personalizzato"],
    CustomHint => [
        "Drag a point to shape your sound (±6 dB). Changes are sent to the headphones as you go.",
        "Faites glisser un point pour façonner votre son (±6 dB). Les changements partent au casque en direct.",
        "Arrastra un punto para moldear tu sonido (±6 dB). Los cambios se envían al momento.",
        "Ziehe einen Punkt, um deinen Klang zu formen (±6 dB). Änderungen gehen sofort an den Kopfhörer.",
        "Trascina un punto per modellare il suono (±6 dB). Le modifiche arrivano subito alle cuffie.",
    ],
    Reset => ["Reset", "Réinitialiser", "Restablecer", "Zurücksetzen", "Ripristina"],
    YourCurve => ["Your curve", "Votre courbe", "Tu curva", "Deine Kurve", "La tua curva"],

    // ---- settings page -------------------------------------------------------------
    SecDevice => ["Device", "Appareil", "Dispositivo", "Gerät", "Dispositivo"],
    DeviceName => ["Name", "Nom", "Nombre", "Name", "Nome"],
    Model => ["Model", "Modèle", "Modelo", "Modell", "Modello"],
    Serial => ["Serial number", "Numéro de série", "Número de serie", "Seriennummer", "Numero di serie"],
    Address => ["Bluetooth address", "Adresse Bluetooth", "Dirección Bluetooth", "Bluetooth-Adresse", "Indirizzo Bluetooth"],
    Refresh => ["Refresh", "Actualiser", "Actualizar", "Aktualisieren", "Aggiorna"],
    ViewLog => ["View log", "Voir le journal", "Ver registro", "Protokoll", "Vedi registro"],
    SecApp => ["App", "Application", "Aplicación", "App", "App"],
    Language => ["Language", "Langue", "Idioma", "Sprache", "Lingua"],
    SystemLanguage => ["System", "Système", "Sistema", "System", "Sistema"],
    Appearance => ["Appearance", "Apparence", "Apariencia", "Erscheinungsbild", "Aspetto"],
    ThemeSystem => ["System", "Système", "Sistema", "System", "Sistema"],
    ThemeLight => ["Light", "Clair", "Claro", "Hell", "Chiaro"],
    ThemeDark => ["Dark", "Sombre", "Oscuro", "Dunkel", "Scuro"],
    SecAbout => ["About", "À propos", "Acerca de", "Über", "Informazioni"],
    Version => ["Version", "Version", "Versión", "Version", "Versione"],
    Disclaimer => [
        "Unofficial, non-certified app. Not affiliated with, endorsed or certified by Anker Innovations or soundcore.",
        "Application non officielle et non homologuée, ni affiliée ni approuvée par Anker Innovations ou soundcore.",
        "App no oficial y no homologada, sin afiliación ni aprobación de Anker Innovations o soundcore.",
        "Inoffizielle, nicht zertifizierte App, weder verbunden mit noch unterstützt von Anker Innovations oder soundcore.",
        "App non ufficiale e non certificata, non affiliata né approvata da Anker Innovations o soundcore.",
    ],
    Credits => [
        "Protocol documented by the OpenSCQ30 project (Oppzippy). Thank you!",
        "Protocole documenté par le projet OpenSCQ30 (Oppzippy). Merci !",
        "Protocolo documentado por el proyecto OpenSCQ30 (Oppzippy). ¡Gracias!",
        "Protokoll dokumentiert vom Projekt OpenSCQ30 (Oppzippy). Danke!",
        "Protocollo documentato dal progetto OpenSCQ30 (Oppzippy). Grazie!",
    ],
    Trademarks => [
        "soundcore and Anker are trademarks of Anker Innovations. Licence GPL-3.0-or-later.",
        "soundcore et Anker sont des marques d'Anker Innovations. Licence GPL-3.0-or-later.",
        "soundcore y Anker son marcas de Anker Innovations. Licencia GPL-3.0-or-later.",
        "soundcore und Anker sind Marken von Anker Innovations. Lizenz GPL-3.0-or-later.",
        "soundcore e Anker sono marchi di Anker Innovations. Licenza GPL-3.0-or-later.",
    ],
    Close => ["Close", "Fermer", "Cerrar", "Schließen", "Chiudi"],
    Log => ["Log", "Journal", "Registro", "Protokoll", "Registro"],
    CopyLog => ["Copy", "Copier", "Copiar", "Kopieren", "Copia"],
    Copied => ["Copied", "Copié", "Copiado", "Kopiert", "Copiato"],

    // ---- notices -------------------------------------------------------------------
    NoAck => [
        "Your headphones didn't confirm the change.",
        "Le casque n'a pas confirmé le changement.",
        "Los auriculares no confirmaron el cambio.",
        "Der Kopfhörer hat die Änderung nicht bestätigt.",
        "Le cuffie non hanno confermato la modifica.",
    ],
    Lost => [
        "Connection lost. Reconnecting…",
        "Connexion perdue. Reconnexion…",
        "Conexión perdida. Reconectando…",
        "Verbindung verloren. Neu verbinden…",
        "Connessione persa. Riconnessione…",
    ],
    UnsupportedTitle => ["Model not supported yet", "Modèle pas encore pris en charge", "Modelo aún no compatible", "Modell noch nicht unterstützt", "Modello non ancora supportato"],
    UnsupportedBody => [
        "This version is made for the soundcore Q20i. Battery and noise control may work, the rest may not.",
        "Cette version est faite pour le soundcore Q20i. La batterie et le contrôle du bruit peuvent fonctionner, le reste peut-être pas.",
        "Esta versión está hecha para los soundcore Q20i. La batería y el control de ruido pueden funcionar; el resto, quizá no.",
        "Diese Version ist für den soundcore Q20i gemacht. Akku und Geräuschkontrolle können funktionieren, der Rest vielleicht nicht.",
        "Questa versione è pensata per le soundcore Q20i. Batteria e controllo del rumore possono funzionare, il resto forse no.",
    ],

    // ---- equalizer presets ---------------------------------------------------------
    PresetSignature => ["soundcore Signature", "soundcore Signature", "soundcore Signature", "soundcore Signature", "soundcore Signature"],
    PresetAcoustic => ["Acoustic", "Acoustique", "Acústico", "Akustik", "Acustico"],
    PresetBassBooster => ["Bass Booster", "Basses renforcées", "Refuerzo de graves", "Bass-Booster", "Potenzia bassi"],
    PresetBassReducer => ["Bass Reducer", "Basses réduites", "Reducción de graves", "Bass-Reduzierer", "Riduci bassi"],
    PresetClassical => ["Classical", "Classique", "Clásica", "Klassik", "Classica"],
    PresetPodcast => ["Podcast", "Podcast", "Pódcast", "Podcast", "Podcast"],
    PresetDance => ["Dance", "Dance", "Dance", "Dance", "Dance"],
    PresetDeep => ["Deep", "Profond", "Profundo", "Tief", "Profondo"],
    PresetElectronic => ["Electronic", "Électronique", "Electrónica", "Elektronisch", "Elettronica"],
    PresetFlat => ["Flat", "Plat", "Plano", "Flach", "Piatto"],
    PresetHipHop => ["Hip-hop", "Hip-hop", "Hip-hop", "Hip-Hop", "Hip-hop"],
    PresetJazz => ["Jazz", "Jazz", "Jazz", "Jazz", "Jazz"],
    PresetLatin => ["Latin", "Latino", "Latina", "Latin", "Latino"],
    PresetLounge => ["Lounge", "Lounge", "Lounge", "Lounge", "Lounge"],
    PresetPiano => ["Piano", "Piano", "Piano", "Klavier", "Pianoforte"],
    PresetPop => ["Pop", "Pop", "Pop", "Pop", "Pop"],
    PresetRnb => ["R&B", "R&B", "R&B", "R&B", "R&B"],
    PresetRock => ["Rock", "Rock", "Rock", "Rock", "Rock"],
    PresetSmallSpeakers => ["Small Speakers", "Petits haut-parleurs", "Altavoces pequeños", "Kleine Lautsprecher", "Altoparlanti piccoli"],
    PresetSpokenWord => ["Spoken Word", "Voix parlée", "Voz hablada", "Gesprochenes Wort", "Parlato"],
    PresetTrebleBooster => ["Treble Booster", "Aigus renforcés", "Refuerzo de agudos", "Höhen-Booster", "Potenzia alti"],
    PresetTrebleReducer => ["Treble Reducer", "Aigus réduits", "Reducción de agudos", "Höhen-Reduzierer", "Riduci alti"],
}

/// Display name of a built-in preset.
pub fn preset_name(key: &str) -> &'static str {
    let k = match key {
        "signature" => T::PresetSignature,
        "acoustic" => T::PresetAcoustic,
        "bass_booster" => T::PresetBassBooster,
        "bass_reducer" => T::PresetBassReducer,
        "classical" => T::PresetClassical,
        "podcast" => T::PresetPodcast,
        "dance" => T::PresetDance,
        "deep" => T::PresetDeep,
        "electronic" => T::PresetElectronic,
        "flat" => T::PresetFlat,
        "hip_hop" => T::PresetHipHop,
        "jazz" => T::PresetJazz,
        "latin" => T::PresetLatin,
        "lounge" => T::PresetLounge,
        "piano" => T::PresetPiano,
        "pop" => T::PresetPop,
        "rnb" => T::PresetRnb,
        "rock" => T::PresetRock,
        "small_speakers" => T::PresetSmallSpeakers,
        "spoken_word" => T::PresetSpokenWord,
        "treble_booster" => T::PresetTrebleBooster,
        "treble_reducer" => T::PresetTrebleReducer,
        _ => T::Custom,
    };
    t(k)
}
