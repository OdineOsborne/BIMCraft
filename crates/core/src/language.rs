//! A project's language: the words its drawings print (a stair's UP, a title block's SCALE, a
//! schedule's headings) and the names commands give what they make (levels, views, sheets, rooms).
//! English, Brazilian Portuguese, Spanish or Italian. Names already given stay as they are; the core's
//! messages, which agents read, stay English.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "pt")]
    Portuguese,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "it")]
    Italian,
}

impl Language {
    pub const ALL: [Language; 4] = [Language::English, Language::Portuguese, Language::Spanish, Language::Italian];

    /// Its code in commands and files: en, pt, es, it.
    pub fn code(self) -> &'static str {
        self.of(["en", "pt", "es", "it"])
    }

    /// Its name, in itself.
    pub fn label(self) -> &'static str {
        self.of(["English", "Português (Brasil)", "Español", "Italiano"])
    }

    /// Of one thing in each language, English first, this language's.
    pub fn of<T>(self, [english, portuguese, spanish, italian]: [T; 4]) -> T {
        match self {
            Language::English => english,
            Language::Portuguese => portuguese,
            Language::Spanish => spanish,
            Language::Italian => italian,
        }
    }

    /// A language by its code or its name, or from a locale as systems give one: pt-BR, es_419.UTF-8.
    pub fn from_code(code: &str) -> Option<Language> {
        let code = code.trim().to_lowercase();
        Some(match code.split(['-', '_', '.']).next().unwrap_or_default() {
            "en" | "english" => Language::English,
            "pt" | "portuguese" | "português" => Language::Portuguese,
            "es" | "spanish" | "español" => Language::Spanish,
            "it" | "italian" | "italiano" => Language::Italian,
            _ => return None,
        })
    }

    /// `english`, a word or a name the core writes, in this language.
    pub fn word(self, english: &str) -> &str {
        self.translated(english).unwrap_or(english)
    }

    /// `english` in this language, when it isn't English and the core's words have it.
    pub fn translated(self, english: &str) -> Option<&'static str> {
        if self == Language::English {
            return None;
        }
        WORDS.iter().find(|w| w[0] == english).map(|&w| self.of(w))
    }

    /// Which of `options` (English words) `name` is, in English or in this language.
    pub fn find(self, name: &str, options: &[&'static str]) -> Option<&'static str> {
        let name = name.trim();
        options.iter().copied().find(|o| o.eq_ignore_ascii_case(name) || self.word(o).to_lowercase() == name.to_lowercase())
    }
}

/// The core's words in English, Brazilian Portuguese, Spanish and Italian.
const WORDS: &[[&str; 4]] = &[
    // Phases and the phase filters a project starts with.
    ["Existing", "Existente", "Existente", "Esistente"],
    ["New Construction", "Nova construção", "Nueva construcción", "Nuova costruzione"],
    ["Show All", "Mostrar tudo", "Mostrar todo", "Mostra tutto"],
    ["Show Complete", "Mostrar completo", "Mostrar completo", "Mostra completo"],
    ["Show New", "Mostrar novo", "Mostrar nuevo", "Mostra nuovo"],
    ["Show Previous + Demolished", "Mostrar anterior + demolido", "Mostrar anterior + demolido", "Mostra precedente + demolito"],
    ["Show Previous + New", "Mostrar anterior + novo", "Mostrar anterior + nuevo", "Mostra precedente + nuovo"],
    // Names commands give.
    ["Project", "Projeto", "Proyecto", "Progetto"],
    ["Level", "Nível", "Nivel", "Livello"],
    ["Section", "Corte", "Sección", "Sezione"],
    ["Elevation", "Elevação", "Elevación", "Prospetto"],
    ["North", "Norte", "Norte", "Nord"],
    ["South", "Sul", "Sur", "Sud"],
    ["East", "Leste", "Este", "Est"],
    ["West", "Oeste", "Oeste", "Ovest"],
    ["Unnamed", "Sem nome", "Sin nombre", "Senza nome"],
    ["Room", "Ambiente", "Habitación", "Locale"],
    ["Mass", "Massa", "Masa", "Massa"],
    ["Room Schedule", "Tabela de ambientes", "Tabla de habitaciones", "Abaco dei locali"],
    ["Door Schedule", "Tabela de portas", "Tabla de puertas", "Abaco delle porte"],
    ["Window Schedule", "Tabela de janelas", "Tabla de ventanas", "Abaco delle finestre"],
    ["Wall Schedule", "Tabela de paredes", "Tabla de muros", "Abaco dei muri"],
    ["Floor Schedule", "Tabela de pisos", "Tabla de pisos", "Abaco dei pavimenti"],
    ["Level Schedule", "Tabela de níveis", "Tabla de niveles", "Abaco dei livelli"],
    ["Ceiling Plan", "Planta de forro", "Plano de cielo raso", "Pianta del controsoffitto"],
    ["Copy", "Cópia", "Copia", "Copia"],
    ["Dependent", "Dependente", "Dependiente", "Dipendente"],
    // The materials library a new project has.
    ["Brick", "Tijolo", "Ladrillo", "Mattone"],
    ["Insulation", "Isolamento", "Aislamiento", "Isolante"],
    ["Concrete block", "Bloco de concreto", "Bloque de hormigón", "Blocco di calcestruzzo"],
    ["Plaster", "Reboco", "Enlucido", "Intonaco"],
    ["Concrete", "Concreto", "Hormigón", "Calcestruzzo"],
    ["Screed", "Contrapiso", "Recrecido", "Massetto"],
    ["Timber", "Madeira", "Madera", "Legno"],
    ["Gypsum board", "Placa de gesso", "Placa de yeso", "Cartongesso"],
    ["Ceramic tile", "Cerâmica", "Baldosa cerámica", "Piastrella ceramica"],
    ["Roof tiles", "Telhas", "Tejas", "Tegole"],
    ["Membrane", "Manta impermeável", "Lámina impermeable", "Guaina impermeabile"],
    ["Fabric", "Tecido", "Tela", "Tessuto"],
    ["Porcelain", "Porcelana", "Porcelana", "Porcellana"],
    // The families a project starts with, and their parameters.
    ["Table", "Mesa", "Mesa", "Tavolo"],
    ["Chair", "Cadeira", "Silla", "Sedia"],
    ["Bed", "Cama", "Cama", "Letto"],
    ["Sofa", "Sofá", "Sofá", "Divano"],
    ["Desk", "Escrivaninha", "Escritorio", "Scrivania"],
    ["Wardrobe", "Guarda-roupa", "Armario", "Armadio"],
    ["Base Cabinet", "Armário inferior", "Mueble bajo", "Mobile base"],
    ["WC", "Vaso sanitário", "Inodoro", "Vaso sanitario"],
    ["Washbasin", "Lavatório", "Lavabo", "Lavabo"],
    ["Bathtub", "Banheira", "Bañera", "Vasca da bagno"],
    // The fixtures of the discipline tabs.
    ["Kitchen Sink", "Pia de cozinha", "Fregadero de cocina", "Lavello da cucina"],
    ["Shower", "Chuveiro", "Ducha", "Doccia"],
    ["Floor Drain", "Ralo", "Sumidero de piso", "Piletta a pavimento"],
    ["Elbow 90", "Joelho 90", "Codo 90", "Gomito 90"],
    ["Tee", "Tê", "Te", "Raccordo a T"],
    ["Gate Valve", "Registro de gaveta", "Válvula de compuerta", "Saracinesca"],
    ["Diameter", "Diâmetro", "Diámetro", "Diametro"],
    ["Distribution Board", "Quadro de distribuição", "Cuadro de distribución", "Quadro di distribuzione"],
    ["Ceiling Luminaire", "Luminária de teto", "Luminaria de techo", "Plafoniera"],
    ["Recessed Luminaire", "Luminária embutida", "Luminaria empotrada", "Faretto da incasso"],
    ["Outlet", "Tomada", "Tomacorriente", "Presa"],
    ["Switch", "Interruptor", "Interruptor", "Interruttore"],
    ["Mounting Height", "Altura de instalação", "Altura de montaje", "Altezza di installazione"],
    ["Air Terminal", "Captor", "Terminal aéreo", "Captatore"],
    ["Earthing Electrode", "Haste de aterramento", "Electrodo de puesta a tierra", "Dispersore di terra"],
    ["Inspection Box", "Caixa de inspeção", "Caja de inspección", "Pozzetto di ispezione"],
    ["Data Outlet", "Ponto de dados", "Punto de datos", "Presa dati"],
    ["Voice Outlet", "Ponto de voz", "Punto de voz", "Presa fonia"],
    ["Patch Panel", "Patch panel", "Panel de parcheo", "Patch panel"],
    ["Rack", "Rack", "Rack", "Rack"],
    ["Single Door", "Porta simples", "Puerta simple", "Porta singola"],
    ["Double Door", "Porta dupla", "Puerta doble", "Porta doppia"],
    ["Fixed Window", "Janela fixa", "Ventana fija", "Finestra fissa"],
    ["Casement Window", "Janela de abrir", "Ventana abatible", "Finestra a battente"],
    ["Frame Width", "Largura do batente", "Ancho del marco", "Larghezza del telaio"],
    ["Frame Depth", "Profundidade do batente", "Profundidad del marco", "Profondità del telaio"],
    ["Frame Material", "Material do batente", "Material del marco", "Materiale del telaio"],
    ["Leaf Material", "Material da folha", "Material de la hoja", "Materiale anta"],
    ["Leaf Thickness", "Espessura da folha", "Espesor de la hoja", "Spessore anta"],
    ["Glass", "Vidro", "Vidrio", "Vetro"],
    ["Metal", "Metal", "Metal", "Metallo"],
    ["PVC", "PVC", "PVC", "PVC"],
    ["Copper", "Cobre", "Cobre", "Rame"],
    ["Depth", "Profundidade", "Profundidad", "Profondità"],
    ["Seat Height", "Altura do assento", "Altura del asiento", "Altezza della seduta"],
    ["Leg Width", "Largura da perna", "Ancho de la pata", "Larghezza della gamba"],
    ["Arm Width", "Largura do braço", "Ancho del brazo", "Larghezza del bracciolo"],
    ["Headboard Height", "Altura da cabeceira", "Altura del cabecero", "Altezza della testiera"],
    ["Worktop", "Bancada", "Encimera", "Piano di lavoro"],
    // What drawings print.
    ["UP", "SOBE", "SUBE", "SALE"],
    ["DN", "DESCE", "BAJA", "SCENDE"],
    ["NOT ENCLOSED", "NÃO DELIMITADO", "NO DELIMITADA", "NON DELIMITATO"],
    ["SCALE", "ESCALA", "ESCALA", "SCALA"],
    ["SCALE AS INDICATED", "ESCALA CONFORME INDICADO", "ESCALA SEGÚN SE INDICA", "SCALA COME INDICATO"],
    // Schedule headings.
    ["Number", "Número", "Número", "Numero"],
    ["Name", "Nome", "Nombre", "Nome"],
    ["Mark", "Marca", "Marca", "Contrassegno"],
    ["Type", "Tipo", "Tipo", "Tipo"],
    ["Width", "Largura", "Ancho", "Larghezza"],
    ["Height", "Altura", "Altura", "Altezza"],
    ["Sill", "Peitoril", "Antepecho", "Davanzale"],
    ["Wall", "Parede", "Muro", "Muro"],
    ["Length", "Comprimento", "Longitud", "Lunghezza"],
    ["Thickness", "Espessura", "Espesor", "Spessore"],
    ["Area", "Área", "Área", "Area"],
    ["Perimeter", "Perímetro", "Perímetro", "Perimetro"],
    ["Volume", "Volume", "Volumen", "Volume"],
    ["Gross Floor Area", "Área bruta de piso", "Superficie bruta de planta", "Superficie lorda di pavimento"],
    ["Count", "Contagem", "Recuento", "Conteggio"],
    ["Family", "Família", "Familia", "Famiglia"],
    ["Offset", "Deslocamento", "Desfase", "Offset"],
    ["Slope", "Inclinação", "Pendiente", "Pendenza"],
    ["Plan Area", "Área em planta", "Área en planta", "Area in pianta"],
    ["Risers", "Espelhos", "Contrahuellas", "Alzate"],
    ["Riser Height", "Altura do espelho", "Altura de contrahuella", "Altezza alzata"],
    ["Tread", "Piso", "Huella", "Pedata"],
    ["Total", "Total", "Total", "Totale"],
    // The parameter that carries a price list's article code.
    ["Article Code", "Código do artigo", "Código de artículo", "Codice articolo"],
    ["Yes", "Sim", "Sí", "Sì"],
    ["No", "Não", "No", "No"],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_come_in_the_projects_language() {
        let pt = Language::Portuguese;
        assert_eq!([pt.word("Level"), pt.word("UP"), pt.word("SCALE AS INDICATED")], ["Nível", "SOBE", "ESCALA CONFORME INDICADO"]);
        assert_eq!(Language::English.word("Level"), "Level");
        assert_eq!(pt.word("Id"), "Id", "a word with no translation stays");
        assert_eq!(pt.find("tabela de ambientes", &["Room Schedule", "Door Schedule"]), Some("Room Schedule"));
        assert_eq!(pt.find("Door Schedule", &["Room Schedule", "Door Schedule"]), Some("Door Schedule"));
        let es = Language::Spanish;
        assert_eq!([es.word("Level"), es.word("UP"), es.word("Room Schedule")], ["Nivel", "SUBE", "Tabla de habitaciones"]);
        assert_eq!(es.find("TABLA DE PUERTAS", &["Room Schedule", "Door Schedule"]), Some("Door Schedule"));
        // Codes as commands write them, and locales as systems give them.
        assert_eq!(
            ["pt-BR", "pt_BR.UTF-8", "es", "es-419", "Español", "en-US", "en", "de", "C", ""].map(Language::from_code),
            [Some(pt), Some(pt), Some(es), Some(es), Some(es), Some(Language::English), Some(Language::English), None, None, None]
        );
    }
}
