//! A small expression language for building vertex/edge sets (and from them
//! new graphs) via set algebra and set-builder notation with quantifiers,
//! e.g.:
//!
//!   NichtIsoliert = { v : v in V(G1), exists u in V(G1) . (v,u) in E(G1) }
//!   G' = (V(G1) & V(G2), E(G1) & E(G2))
//!
//! Both ASCII/word operators (`|`, `&`, `-`, `^`, `in`, `forall`, ...) and
//! their Unicode math equivalents (`∪`, `∩`, `\`, `△`, `∈`, `∀`, ...) are
//! accepted as synonyms; the ASCII forms are guaranteed to render in the
//! UI's font, so they are what's documented as the primary syntax.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::graph::{Edge, Graph, VertexId};

// ---------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum SetValue {
    Vertices(BTreeSet<VertexId>),
    Edges(BTreeSet<Edge>),
}

impl SetValue {
    pub fn kind(&self) -> &'static str {
        match self {
            SetValue::Vertices(_) => "Knotenmenge",
            SetValue::Edges(_) => "Kantenmenge",
        }
    }

    pub fn len(&self) -> usize {
        match self {
            SetValue::Vertices(s) => s.len(),
            SetValue::Edges(s) => s.len(),
        }
    }

    pub fn format(&self) -> String {
        match self {
            SetValue::Vertices(s) => {
                let items: Vec<String> = s.iter().cloned().collect();
                format!("{{{}}}", items.join(", "))
            }
            SetValue::Edges(s) => {
                let items: Vec<String> = s.iter().map(|(a, b)| format!("({a},{b})")).collect();
                format!("{{{}}}", items.join(", "))
            }
        }
    }
}

// ---------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Eq,
    Ne,
    Union,
    Intersect,
    Diff,
    SymDiff,
    And,
    Or,
    Not,
    In,
    NotIn,
    Subset,
    ForAll,
    Exists,
    SuchThat,
    EmptySet,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut toks = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        match c {
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '{' => {
                toks.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                toks.push(Tok::RBrace);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            ':' | '.' => {
                toks.push(Tok::SuchThat);
                i += 1;
            }
            '|' => {
                if chars.get(i + 1) == Some(&'|') {
                    toks.push(Tok::Or);
                    i += 2;
                } else {
                    toks.push(Tok::Union);
                    i += 1;
                }
            }
            '&' => {
                if chars.get(i + 1) == Some(&'&') {
                    toks.push(Tok::And);
                    i += 2;
                } else {
                    toks.push(Tok::Intersect);
                    i += 1;
                }
            }
            '-' | '\\' => {
                toks.push(Tok::Diff);
                i += 1;
            }
            '^' => {
                toks.push(Tok::SymDiff);
                i += 1;
            }
            '!' => {
                if chars.get(i + 1) == Some(&'=') {
                    toks.push(Tok::Ne);
                    i += 2;
                } else {
                    toks.push(Tok::Not);
                    i += 1;
                }
            }
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    i += 2;
                } else {
                    i += 1;
                }
                toks.push(Tok::Eq);
            }
            '<' => {
                if chars.get(i + 1) == Some(&'=') {
                    toks.push(Tok::Subset);
                    i += 2;
                } else {
                    return Err(format!("Unerwartetes Zeichen '<' an Position {i}"));
                }
            }
            '∪' => {
                toks.push(Tok::Union);
                i += 1;
            }
            '∩' => {
                toks.push(Tok::Intersect);
                i += 1;
            }
            '∖' => {
                toks.push(Tok::Diff);
                i += 1;
            }
            '△' => {
                toks.push(Tok::SymDiff);
                i += 1;
            }
            '∧' => {
                toks.push(Tok::And);
                i += 1;
            }
            '∨' => {
                toks.push(Tok::Or);
                i += 1;
            }
            '¬' => {
                toks.push(Tok::Not);
                i += 1;
            }
            '∈' => {
                toks.push(Tok::In);
                i += 1;
            }
            '∉' => {
                toks.push(Tok::NotIn);
                i += 1;
            }
            '⊆' => {
                toks.push(Tok::Subset);
                i += 1;
            }
            '∀' => {
                toks.push(Tok::ForAll);
                i += 1;
            }
            '∃' => {
                toks.push(Tok::Exists);
                i += 1;
            }
            '≠' => {
                toks.push(Tok::Ne);
                i += 1;
            }
            '∅' => {
                toks.push(Tok::EmptySet);
                i += 1;
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match word.as_str() {
                    "in" => toks.push(Tok::In),
                    "notin" => toks.push(Tok::NotIn),
                    "and" => toks.push(Tok::And),
                    "or" => toks.push(Tok::Or),
                    "not" => toks.push(Tok::Not),
                    "xor" => toks.push(Tok::SymDiff),
                    "forall" => toks.push(Tok::ForAll),
                    "exists" => toks.push(Tok::Exists),
                    "empty" => toks.push(Tok::EmptySet),
                    _ => toks.push(Tok::Ident(word)),
                }
            }
            other => return Err(format!("Unerwartetes Zeichen '{other}'")),
        }
    }
    Ok(toks)
}

// ---------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum SetExpr {
    Name(String),
    GraphVertices(String),
    GraphEdges(String),
    Empty,
    Union(Box<SetExpr>, Box<SetExpr>),
    Intersect(Box<SetExpr>, Box<SetExpr>),
    Diff(Box<SetExpr>, Box<SetExpr>),
    SymDiff(Box<SetExpr>, Box<SetExpr>),
    Comprehension { output: Output, quals: Vec<Qual> },
}

#[derive(Clone, Debug)]
pub enum Output {
    Var(String),
    Pair(String, String),
}

#[derive(Clone, Debug)]
pub enum Qual {
    Binder(String, SetExpr),
    Guard(BoolExpr),
}

#[derive(Clone, Debug)]
pub enum Term {
    Var(String),
    Pair(String, String),
}

#[derive(Clone, Debug)]
pub enum BoolExpr {
    Eq(Term, Term),
    Ne(Term, Term),
    MemberOf(Term, SetExpr),
    NotMemberOf(Term, SetExpr),
    Subset(SetExpr, SetExpr),
    And(Box<BoolExpr>, Box<BoolExpr>),
    Or(Box<BoolExpr>, Box<BoolExpr>),
    Not(Box<BoolExpr>),
    ForAll(String, SetExpr, Box<BoolExpr>),
    Exists(String, SetExpr, Box<BoolExpr>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    DefineSet(String, SetExpr),
    DefineGraph(String, SetExpr, SetExpr),
}

// ---------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn new(toks: Vec<Tok>) -> Self {
        Self { toks, pos: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn advance(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn check(&self, t: &Tok) -> bool {
        self.peek() == Some(t)
    }

    fn expect(&mut self, t: Tok) -> Result<(), String> {
        if self.check(&t) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("Erwarte {t:?}, gefunden {:?}", self.peek()))
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.advance() {
            Some(Tok::Ident(s)) => Ok(s),
            other => Err(format!("Bezeichner erwartet, gefunden {other:?}")),
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    fn looks_like_pair_term(&self) -> bool {
        matches!(self.toks.get(self.pos), Some(Tok::LParen))
            && matches!(self.toks.get(self.pos + 1), Some(Tok::Ident(_)))
            && matches!(self.toks.get(self.pos + 2), Some(Tok::Comma))
    }

    // --- set expressions (union loosest, then diff/symdiff, then intersect) ---

    fn parse_set_expr(&mut self) -> Result<SetExpr, String> {
        self.parse_union()
    }

    fn parse_union(&mut self) -> Result<SetExpr, String> {
        let mut lhs = self.parse_diffsym()?;
        while self.check(&Tok::Union) {
            self.advance();
            let rhs = self.parse_diffsym()?;
            lhs = SetExpr::Union(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_diffsym(&mut self) -> Result<SetExpr, String> {
        let mut lhs = self.parse_intersect()?;
        loop {
            if self.check(&Tok::Diff) {
                self.advance();
                let rhs = self.parse_intersect()?;
                lhs = SetExpr::Diff(Box::new(lhs), Box::new(rhs));
            } else if self.check(&Tok::SymDiff) {
                self.advance();
                let rhs = self.parse_intersect()?;
                lhs = SetExpr::SymDiff(Box::new(lhs), Box::new(rhs));
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_intersect(&mut self) -> Result<SetExpr, String> {
        let mut lhs = self.parse_primary_set()?;
        while self.check(&Tok::Intersect) {
            self.advance();
            let rhs = self.parse_primary_set()?;
            lhs = SetExpr::Intersect(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_primary_set(&mut self) -> Result<SetExpr, String> {
        match self.peek().cloned() {
            Some(Tok::EmptySet) => {
                self.advance();
                Ok(SetExpr::Empty)
            }
            Some(Tok::LBrace) => {
                self.advance();
                if self.check(&Tok::RBrace) {
                    self.advance();
                    return Ok(SetExpr::Empty);
                }
                let comp = self.parse_comprehension()?;
                Ok(comp)
            }
            Some(Tok::LParen) => {
                self.advance();
                let inner = self.parse_set_expr()?;
                self.expect(Tok::RParen)?;
                Ok(inner)
            }
            Some(Tok::Ident(name)) => {
                self.advance();
                if (name == "V" || name == "E") && self.check(&Tok::LParen) {
                    self.advance();
                    let g = self.expect_ident()?;
                    self.expect(Tok::RParen)?;
                    if name == "V" {
                        Ok(SetExpr::GraphVertices(g))
                    } else {
                        Ok(SetExpr::GraphEdges(g))
                    }
                } else {
                    Ok(SetExpr::Name(name))
                }
            }
            other => Err(format!("Menge erwartet, gefunden {other:?}")),
        }
    }

    fn parse_comprehension(&mut self) -> Result<SetExpr, String> {
        let output = self.parse_output()?;
        self.expect(Tok::SuchThat)?;
        let mut quals = Vec::new();
        loop {
            quals.push(self.parse_qual()?);
            if self.check(&Tok::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(Tok::RBrace)?;
        Ok(SetExpr::Comprehension { output, quals })
    }

    fn parse_output(&mut self) -> Result<Output, String> {
        if self.check(&Tok::LParen) {
            self.advance();
            let a = self.expect_ident()?;
            self.expect(Tok::Comma)?;
            let b = self.expect_ident()?;
            self.expect(Tok::RParen)?;
            Ok(Output::Pair(a, b))
        } else {
            Ok(Output::Var(self.expect_ident()?))
        }
    }

    fn parse_qual(&mut self) -> Result<Qual, String> {
        if let Some(Tok::Ident(name)) = self.peek().cloned() {
            if self.toks.get(self.pos + 1) == Some(&Tok::In) {
                self.advance();
                self.advance();
                let dom = self.parse_set_expr()?;
                return Ok(Qual::Binder(name, dom));
            }
        }
        Ok(Qual::Guard(self.parse_bool_expr()?))
    }

    // --- boolean expressions ---

    fn parse_bool_expr(&mut self) -> Result<BoolExpr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<BoolExpr, String> {
        let mut lhs = self.parse_and()?;
        while self.check(&Tok::Or) {
            self.advance();
            let rhs = self.parse_and()?;
            lhs = BoolExpr::Or(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<BoolExpr, String> {
        let mut lhs = self.parse_not()?;
        while self.check(&Tok::And) {
            self.advance();
            let rhs = self.parse_not()?;
            lhs = BoolExpr::And(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_not(&mut self) -> Result<BoolExpr, String> {
        if self.check(&Tok::Not) {
            self.advance();
            return Ok(BoolExpr::Not(Box::new(self.parse_not()?)));
        }
        self.parse_quant()
    }

    fn parse_quant(&mut self) -> Result<BoolExpr, String> {
        if self.check(&Tok::ForAll) || self.check(&Tok::Exists) {
            let is_forall = self.check(&Tok::ForAll);
            self.advance();
            let var = self.expect_ident()?;
            self.expect(Tok::In)?;
            let dom = self.parse_set_expr()?;
            self.expect(Tok::SuchThat)?;
            let body = self.parse_bool_expr()?;
            return Ok(if is_forall {
                BoolExpr::ForAll(var, dom, Box::new(body))
            } else {
                BoolExpr::Exists(var, dom, Box::new(body))
            });
        }
        self.parse_atom_bool()
    }

    fn parse_atom_bool(&mut self) -> Result<BoolExpr, String> {
        if self.looks_like_pair_term() {
            let term = self.parse_term()?;
            return self.parse_atom_bool_after_term(term);
        }
        if self.check(&Tok::LParen) {
            let save = self.pos;
            self.advance();
            if let Ok(inner) = self.parse_bool_expr() {
                if self.check(&Tok::RParen) {
                    self.advance();
                    return Ok(inner);
                }
            }
            self.pos = save;
        }
        let lhs_set = self.parse_set_expr()?;
        match self.peek().cloned() {
            Some(Tok::Subset) => {
                self.advance();
                let rhs = self.parse_set_expr()?;
                Ok(BoolExpr::Subset(lhs_set, rhs))
            }
            Some(Tok::In) | Some(Tok::NotIn) | Some(Tok::Eq) | Some(Tok::Ne) => {
                let term = set_expr_to_term(lhs_set)?;
                self.parse_atom_bool_after_term(term)
            }
            other => Err(format!(
                "Erwarte 'in' / '=' / '!=' / '<=' , gefunden {other:?}"
            )),
        }
    }

    fn parse_atom_bool_after_term(&mut self, term: Term) -> Result<BoolExpr, String> {
        match self.peek().cloned() {
            Some(Tok::In) => {
                self.advance();
                Ok(BoolExpr::MemberOf(term, self.parse_set_expr()?))
            }
            Some(Tok::NotIn) => {
                self.advance();
                Ok(BoolExpr::NotMemberOf(term, self.parse_set_expr()?))
            }
            Some(Tok::Eq) => {
                self.advance();
                Ok(BoolExpr::Eq(term, self.parse_term()?))
            }
            Some(Tok::Ne) => {
                self.advance();
                Ok(BoolExpr::Ne(term, self.parse_term()?))
            }
            other => Err(format!(
                "Erwarte 'in' / 'notin' / '=' / '!=' nach Term, gefunden {other:?}"
            )),
        }
    }

    fn parse_term(&mut self) -> Result<Term, String> {
        if self.check(&Tok::LParen) {
            self.advance();
            let a = self.expect_ident()?;
            self.expect(Tok::Comma)?;
            let b = self.expect_ident()?;
            self.expect(Tok::RParen)?;
            Ok(Term::Pair(a, b))
        } else {
            Ok(Term::Var(self.expect_ident()?))
        }
    }
}

fn set_expr_to_term(e: SetExpr) -> Result<Term, String> {
    match e {
        SetExpr::Name(n) => Ok(Term::Var(n)),
        _ => Err(
            "Links von 'in' / '=' / '!=' wird eine einzelne (gebundene) Variable erwartet, \
             keine zusammengesetzte Mengenoperation."
                .to_string(),
        ),
    }
}

/// Parses one statement of the form `Name = SetExpr` or
/// `Name = (VertexExpr, EdgeExpr)`.
pub fn parse_statement(input: &str) -> Result<Stmt, String> {
    let toks = tokenize(input)?;
    if toks.is_empty() {
        return Err("Leere Eingabe".to_string());
    }
    let mut p = Parser::new(toks);
    let name = p
        .expect_ident()
        .map_err(|_| "Erwarte 'Name = Ausdruck' am Zeilenanfang".to_string())?;
    p.expect(Tok::Eq)?;

    if p.check(&Tok::LParen) {
        let save = p.pos;
        p.advance();
        if let Ok(first) = p.parse_set_expr() {
            if p.check(&Tok::Comma) {
                p.advance();
                let second = p.parse_set_expr()?;
                p.expect(Tok::RParen)?;
                if !p.at_end() {
                    return Err("Unerwartete Zeichen nach dem Graph-Ausdruck (V, E)".to_string());
                }
                return Ok(Stmt::DefineGraph(name, first, second));
            } else if p.check(&Tok::RParen) {
                p.advance();
                if !p.at_end() {
                    return Err(
                        "Unerwartete Zeichen nach '(...)': ein Ausdruck darf nicht mit einer \
                         geklammerten Menge beginnen und danach weitere Operatoren haben — \
                         Klammer bitte nicht an den Anfang stellen."
                            .to_string(),
                    );
                }
                return Ok(Stmt::DefineSet(name, first));
            }
        }
        p.pos = save;
    }

    let expr = p.parse_set_expr()?;
    if !p.at_end() {
        return Err(format!("Unerwartete Zeichen am Ende: {:?}", p.peek()));
    }
    Ok(Stmt::DefineSet(name, expr))
}

// ---------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------

pub struct EvalCtx<'a> {
    pub graphs: BTreeMap<&'a str, &'a Graph>,
    pub named: &'a BTreeMap<String, SetValue>,
}

const MAX_COMPREHENSION_STATES: usize = 200_000;

pub fn eval_set(expr: &SetExpr, ctx: &EvalCtx) -> Result<SetValue, String> {
    match expr {
        SetExpr::Name(n) => ctx
            .named
            .get(n)
            .cloned()
            .ok_or_else(|| format!("Unbekannte Menge \"{n}\"")),
        SetExpr::GraphVertices(g) => ctx
            .graphs
            .get(g.as_str())
            .map(|gr| SetValue::Vertices(gr.vertices.clone()))
            .ok_or_else(|| format!("Unbekannter Graph \"{g}\"")),
        SetExpr::GraphEdges(g) => ctx
            .graphs
            .get(g.as_str())
            .map(|gr| SetValue::Edges(gr.edges.clone()))
            .ok_or_else(|| format!("Unbekannter Graph \"{g}\"")),
        SetExpr::Empty => Ok(SetValue::Vertices(BTreeSet::new())),
        SetExpr::Union(a, b) => {
            let (x, y) = (eval_set(a, ctx)?, eval_set(b, ctx)?);
            binary_op(
                x,
                y,
                "∪",
                |p, q| p.union(q).cloned().collect(),
                |p, q| p.union(q).cloned().collect(),
            )
        }
        SetExpr::Intersect(a, b) => {
            let (x, y) = (eval_set(a, ctx)?, eval_set(b, ctx)?);
            binary_op(
                x,
                y,
                "∩",
                |p, q| p.intersection(q).cloned().collect(),
                |p, q| p.intersection(q).cloned().collect(),
            )
        }
        SetExpr::Diff(a, b) => {
            let (x, y) = (eval_set(a, ctx)?, eval_set(b, ctx)?);
            binary_op(
                x,
                y,
                "\\",
                |p, q| p.difference(q).cloned().collect(),
                |p, q| p.difference(q).cloned().collect(),
            )
        }
        SetExpr::SymDiff(a, b) => {
            let (x, y) = (eval_set(a, ctx)?, eval_set(b, ctx)?);
            binary_op(
                x,
                y,
                "^",
                |p, q| p.symmetric_difference(q).cloned().collect(),
                |p, q| p.symmetric_difference(q).cloned().collect(),
            )
        }
        SetExpr::Comprehension { output, quals } => eval_comprehension(output, quals, ctx),
    }
}

fn binary_op(
    a: SetValue,
    b: SetValue,
    op_name: &str,
    vop: impl Fn(&BTreeSet<VertexId>, &BTreeSet<VertexId>) -> BTreeSet<VertexId>,
    eop: impl Fn(&BTreeSet<Edge>, &BTreeSet<Edge>) -> BTreeSet<Edge>,
) -> Result<SetValue, String> {
    use SetValue::*;
    match (a, b) {
        (Vertices(x), Vertices(y)) => Ok(Vertices(vop(&x, &y))),
        (Edges(x), Edges(y)) => Ok(Edges(eop(&x, &y))),
        (Vertices(x), Edges(y)) if x.is_empty() => Ok(Edges(eop(&BTreeSet::new(), &y))),
        (Edges(x), Vertices(y)) if y.is_empty() => Ok(Edges(eop(&x, &BTreeSet::new()))),
        (x, y) => Err(format!(
            "Typfehler bei '{op_name}': {} und {} passen nicht zusammen",
            x.kind(),
            y.kind()
        )),
    }
}

#[derive(Clone, PartialEq)]
enum Bound {
    Vertex(VertexId),
    Edge(Edge),
}

type Env = BTreeMap<String, Bound>;

fn eval_comprehension(output: &Output, quals: &[Qual], ctx: &EvalCtx) -> Result<SetValue, String> {
    let mut envs: Vec<Env> = vec![Env::new()];
    for q in quals {
        match q {
            Qual::Binder(var, domain) => {
                if envs.iter().any(|e| e.contains_key(var)) {
                    return Err(format!("Variable \"{var}\" ist bereits gebunden"));
                }
                let dom = eval_set(domain, ctx)?;
                let mut next = Vec::new();
                for env in &envs {
                    for b in bound_iter(&dom) {
                        let mut e2 = env.clone();
                        e2.insert(var.clone(), b);
                        next.push(e2);
                    }
                }
                if next.len() > MAX_COMPREHENSION_STATES {
                    return Err(
                        "Ausdruck zu groß (zu viele Kombinationen) — bitte einschränken"
                            .to_string(),
                    );
                }
                envs = next;
            }
            Qual::Guard(pred) => {
                let mut kept = Vec::with_capacity(envs.len());
                for env in envs {
                    if eval_bool(pred, ctx, &env)? {
                        kept.push(env);
                    }
                }
                envs = kept;
            }
        }
    }

    match output {
        Output::Var(name) => {
            let mut verts = BTreeSet::new();
            let mut edges = BTreeSet::new();
            let mut is_edge = false;
            for env in &envs {
                match env.get(name) {
                    Some(Bound::Vertex(v)) => {
                        verts.insert(v.clone());
                    }
                    Some(Bound::Edge(e)) => {
                        edges.insert(e.clone());
                        is_edge = true;
                    }
                    None => {
                        return Err(format!(
                            "Variable \"{name}\" ist im Ausgabeausdruck nicht gebunden"
                        ))
                    }
                }
            }
            Ok(if is_edge {
                SetValue::Edges(edges)
            } else {
                SetValue::Vertices(verts)
            })
        }
        Output::Pair(a, b) => {
            let mut edges = BTreeSet::new();
            for env in &envs {
                let av = require_vertex(env, a)?;
                let bv = require_vertex(env, b)?;
                edges.insert((av, bv));
            }
            Ok(SetValue::Edges(edges))
        }
    }
}

fn bound_iter(s: &SetValue) -> Vec<Bound> {
    match s {
        SetValue::Vertices(vs) => vs.iter().cloned().map(Bound::Vertex).collect(),
        SetValue::Edges(es) => es.iter().cloned().map(Bound::Edge).collect(),
    }
}

fn require_vertex(env: &Env, name: &str) -> Result<VertexId, String> {
    match env.get(name) {
        Some(Bound::Vertex(v)) => Ok(v.clone()),
        Some(Bound::Edge(_)) => Err(format!(
            "\"{name}\" ist eine Kantenvariable, hier wird ein Knoten erwartet"
        )),
        None => Err(format!("Variable \"{name}\" ist nicht gebunden")),
    }
}

fn term_value(term: &Term, env: &Env) -> Result<Bound, String> {
    match term {
        Term::Var(n) => env
            .get(n)
            .cloned()
            .ok_or_else(|| format!("Variable \"{n}\" ist nicht gebunden")),
        Term::Pair(a, b) => {
            let av = require_vertex(env, a)?;
            let bv = require_vertex(env, b)?;
            Ok(Bound::Edge((av, bv)))
        }
    }
}

fn eval_bool(pred: &BoolExpr, ctx: &EvalCtx, env: &Env) -> Result<bool, String> {
    match pred {
        BoolExpr::Eq(a, b) => Ok(term_value(a, env)? == term_value(b, env)?),
        BoolExpr::Ne(a, b) => Ok(term_value(a, env)? != term_value(b, env)?),
        BoolExpr::MemberOf(t, se) => {
            let v = term_value(t, env)?;
            let s = eval_set(se, ctx)?;
            member(&v, &s)
        }
        BoolExpr::NotMemberOf(t, se) => {
            let v = term_value(t, env)?;
            let s = eval_set(se, ctx)?;
            member(&v, &s).map(|b| !b)
        }
        BoolExpr::Subset(a, b) => {
            let (sa, sb) = (eval_set(a, ctx)?, eval_set(b, ctx)?);
            subset(&sa, &sb)
        }
        BoolExpr::And(a, b) => Ok(eval_bool(a, ctx, env)? && eval_bool(b, ctx, env)?),
        BoolExpr::Or(a, b) => Ok(eval_bool(a, ctx, env)? || eval_bool(b, ctx, env)?),
        BoolExpr::Not(a) => Ok(!eval_bool(a, ctx, env)?),
        BoolExpr::ForAll(var, dom, body) => {
            if env.contains_key(var) {
                return Err(format!("Variable \"{var}\" ist bereits gebunden"));
            }
            let d = eval_set(dom, ctx)?;
            for elem in bound_iter(&d) {
                let mut e2 = env.clone();
                e2.insert(var.clone(), elem);
                if !eval_bool(body, ctx, &e2)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        BoolExpr::Exists(var, dom, body) => {
            if env.contains_key(var) {
                return Err(format!("Variable \"{var}\" ist bereits gebunden"));
            }
            let d = eval_set(dom, ctx)?;
            for elem in bound_iter(&d) {
                let mut e2 = env.clone();
                e2.insert(var.clone(), elem);
                if eval_bool(body, ctx, &e2)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn member(v: &Bound, s: &SetValue) -> Result<bool, String> {
    match (v, s) {
        (Bound::Vertex(x), SetValue::Vertices(set)) => Ok(set.contains(x)),
        (Bound::Edge(x), SetValue::Edges(set)) => Ok(set.contains(x)),
        _ => Err(
            "Typfehler: Knoten können nicht in einer Kantenmenge enthalten sein (und umgekehrt)"
                .to_string(),
        ),
    }
}

fn subset(a: &SetValue, b: &SetValue) -> Result<bool, String> {
    match (a, b) {
        (SetValue::Vertices(x), SetValue::Vertices(y)) => Ok(x.is_subset(y)),
        (SetValue::Edges(x), SetValue::Edges(y)) => Ok(x.is_subset(y)),
        (SetValue::Vertices(x), SetValue::Edges(_)) if x.is_empty() => Ok(true),
        (SetValue::Edges(x), SetValue::Vertices(_)) if x.is_empty() => Ok(true),
        (a, b) => Err(format!(
            "Typfehler bei '<=': {} und {} passen nicht zusammen",
            a.kind(),
            b.kind()
        )),
    }
}

// ---------------------------------------------------------------------
// Statement execution
// ---------------------------------------------------------------------

pub enum StmtResult {
    Set(String, SetValue),
    Graph(String, BTreeSet<VertexId>, BTreeSet<Edge>),
}

pub fn eval_statement(stmt: &Stmt, ctx: &EvalCtx) -> Result<StmtResult, String> {
    match stmt {
        Stmt::DefineSet(name, expr) => {
            let v = eval_set(expr, ctx)?;
            Ok(StmtResult::Set(name.clone(), v))
        }
        Stmt::DefineGraph(name, vexpr, eexpr) => {
            let v = eval_set(vexpr, ctx)?;
            let e = eval_set(eexpr, ctx)?;
            let verts =
                match v {
                    SetValue::Vertices(s) => s,
                    SetValue::Edges(s) if s.is_empty() => BTreeSet::new(),
                    SetValue::Edges(_) => return Err(
                        "Der erste Teil von (V, E) muss eine Knotenmenge sein, keine Kantenmenge"
                            .to_string(),
                    ),
                };
            let edges =
                match e {
                    SetValue::Edges(s) => s,
                    SetValue::Vertices(s) if s.is_empty() => BTreeSet::new(),
                    SetValue::Vertices(_) => return Err(
                        "Der zweite Teil von (V, E) muss eine Kantenmenge sein, keine Knotenmenge"
                            .to_string(),
                    ),
                };
            Ok(StmtResult::Graph(name.clone(), verts, edges))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> BTreeMap<String, Graph> {
        let mut g1 = Graph::new(false);
        g1.add_edge("v0", "v1");
        g1.add_edge("v1", "v2");
        g1.add_vertex("v3"); // isolated

        let mut g2 = Graph::new(false);
        g2.add_edge("v1", "v2");
        g2.add_edge("v2", "v4");

        let mut m = BTreeMap::new();
        m.insert("G1".to_string(), g1);
        m.insert("G2".to_string(), g2);
        m
    }

    fn ctx<'a>(
        graphs: &'a BTreeMap<String, Graph>,
        named: &'a BTreeMap<String, SetValue>,
    ) -> EvalCtx<'a> {
        EvalCtx {
            graphs: graphs.iter().map(|(k, v)| (k.as_str(), v)).collect(),
            named,
        }
    }

    fn run(input: &str, graphs: &BTreeMap<String, Graph>, named: &mut BTreeMap<String, SetValue>) {
        let stmt = parse_statement(input).expect("parse");
        let c = ctx(graphs, named);
        let result = eval_statement(&stmt, &c).expect("eval");
        if let StmtResult::Set(name, val) = result {
            named.insert(name, val);
        } else {
            panic!("expected a set statement");
        }
    }

    #[test]
    fn union_of_graph_vertex_sets() {
        let graphs = fixture();
        let mut named = BTreeMap::new();
        run("A = V(G1) | V(G2)", &graphs, &mut named);
        let SetValue::Vertices(v) = &named["A"] else {
            panic!()
        };
        assert_eq!(
            v,
            &["v0", "v1", "v2", "v3", "v4"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        );
    }

    #[test]
    fn intersection_of_edge_sets() {
        let graphs = fixture();
        let mut named = BTreeMap::new();
        run("A = E(G1) & E(G2)", &graphs, &mut named);
        let SetValue::Edges(e) = &named["A"] else {
            panic!()
        };
        assert_eq!(e.len(), 1);
        assert!(e.contains(&("v1".to_string(), "v2".to_string())));
    }

    #[test]
    fn filter_comprehension_non_isolated_vertices() {
        let graphs = fixture();
        let mut named = BTreeMap::new();
        run(
            "A = { v : v in V(G1), exists u in V(G1) . (v,u) in E(G1) or (u,v) in E(G1) }",
            &graphs,
            &mut named,
        );
        let SetValue::Vertices(v) = &named["A"] else {
            panic!()
        };
        assert!(v.contains("v0") && v.contains("v1") && v.contains("v2"));
        assert!(!v.contains("v3"));
    }

    #[test]
    fn pair_builder_all_non_edges() {
        let graphs = fixture();
        let mut named = BTreeMap::new();
        run(
            "A = { (u,v) : u in V(G1), v in V(G1), u != v, (u,v) notin E(G1) }",
            &graphs,
            &mut named,
        );
        let SetValue::Edges(e) = &named["A"] else {
            panic!()
        };
        // v0-v1 and v1-v2 exist; complement should include e.g. v0,v2 both directions
        assert!(e.contains(&("v0".to_string(), "v2".to_string())));
        assert!(e.contains(&("v2".to_string(), "v0".to_string())));
        assert!(!e.contains(&("v0".to_string(), "v1".to_string())));
    }

    #[test]
    fn forall_quantifier() {
        let graphs = fixture();
        let mut named = BTreeMap::new();
        run(
            "A = { v : v in V(G1), forall u in V(G1) . v != u or v = u }",
            &graphs,
            &mut named,
        );
        // trivially true tautology -> all vertices of G1 included
        let SetValue::Vertices(v) = &named["A"] else {
            panic!()
        };
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn empty_set_literal_coerces_type() {
        let graphs = fixture();
        let mut named = BTreeMap::new();
        run("A = E(G1) | {}", &graphs, &mut named);
        let SetValue::Edges(e) = &named["A"] else {
            panic!()
        };
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn graph_definition_statement() {
        let graphs = fixture();
        let named = BTreeMap::new();
        let stmt = parse_statement("G3 = (V(G1) & V(G2), E(G1) & E(G2))").expect("parse");
        let c = ctx(&graphs, &named);
        let result = eval_statement(&stmt, &c).expect("eval");
        let StmtResult::Graph(name, verts, edges) = result else {
            panic!("expected graph def")
        };
        assert_eq!(name, "G3");
        assert!(verts.contains("v1") && verts.contains("v2"));
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn type_mismatch_errors() {
        let graphs = fixture();
        let named = BTreeMap::new();
        let stmt = parse_statement("A = V(G1) | E(G1)").unwrap();
        let c = ctx(&graphs, &named);
        assert!(eval_statement(&stmt, &c).is_err());
    }

    #[test]
    fn unknown_name_errors() {
        let graphs = fixture();
        let named = BTreeMap::new();
        let stmt = parse_statement("A = Nope | V(G1)").unwrap();
        let c = ctx(&graphs, &named);
        assert!(eval_statement(&stmt, &c).is_err());
    }

    #[test]
    fn subset_predicate_as_guard() {
        let graphs = fixture();
        let mut named = BTreeMap::new();
        named.insert(
            "Small".to_string(),
            SetValue::Vertices(["v1".to_string(), "v2".to_string()].into_iter().collect()),
        );
        run(
            "A = { v : v in V(G1), Small <= V(G1) }",
            &graphs,
            &mut named,
        );
        let SetValue::Vertices(v) = &named["A"] else {
            panic!()
        };
        assert_eq!(v.len(), 4); // guard true for all v since it doesn't depend on v
    }

    #[test]
    fn chained_statements_reference_earlier_names() {
        let graphs = fixture();
        let mut named = BTreeMap::new();
        run("A = V(G1) - V(G2)", &graphs, &mut named);
        run("B = A | V(G2)", &graphs, &mut named);
        let SetValue::Vertices(b) = &named["B"] else {
            panic!()
        };
        assert_eq!(b.len(), 5);
    }
}
