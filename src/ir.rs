#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Term,
    Io,
    Act,
    Decision,
    Loop,
    Return,
    Conn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopKind {
    While,
    For,
    /// тело выполняется до первой проверки условия: рисуется тем же
    /// шестиугольником, но текст заголовка несёт «do while» — без
    /// подписи цикл врал бы, что проверка идёт первой
    DoWhile,
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub label: String,
    pub stmts: Vec<Stmt>,
    pub to_end: bool,
    pub link: Option<char>,
}

/// Тип обычной плитки: процесс или ввод-вывод (ГОСТ 19.701).
/// Решается один раз — при строительстве IR во frontend'е.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Act,
    Io,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    /// обычная плитка-оператор
    Tile {
        kind: TileKind,
        text: String,
    },
    /// выход из внутреннего цикла или switch: рельса или растворение
    Break,
    /// следующая итерация внутреннего цикла
    Continue,
    /// тупик-плитка return с исходным текстом
    Return(String),
    Node(Box<Node>),
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub text: String,
    pub branches: Vec<Branch>,
    pub body: Option<Vec<Stmt>>,
    pub loop_kind: Option<LoopKind>,
    pub lang: String,
    pub switch_var: Option<String>,
}

impl From<TileKind> for NodeKind {
    fn from(k: TileKind) -> Self {
        match k {
            TileKind::Io => NodeKind::Io,
            TileKind::Act => NodeKind::Act,
        }
    }
}

impl Node {
    /// Текст блока «подготовка»: ключевое слово цикла + условие,
    /// чтобы на схеме было видно, какой это цикл.
    pub fn loop_label(&self) -> String {
        match self.loop_kind {
            Some(LoopKind::While) => format!("while {}", self.text),
            Some(LoopKind::For) => format!("for {}", self.text),
            // проверка идёт после тела — слово «do» обязательно, иначе
            // шестиугольник врёт, что проверка первая
            Some(LoopKind::DoWhile) => format!("do while {}", self.text),
            None => self.text.clone(),
        }
    }

    pub fn new(kind: NodeKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            branches: Vec::new(),
            body: None,
            loop_kind: None,
            lang: "en".to_string(),
            switch_var: None,
        }
    }
}
