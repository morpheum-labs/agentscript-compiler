//! Type syntax and variable qualifiers.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarQualifier {
    Var,
    Varip,
    Const,
    Input,
    Simple,
    Series,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Primitive(PrimitiveType),
    /// User `enum` or `type` name, e.g. `map<symbols, float>`.
    Named(String),
    Array(Box<Type>),
    Matrix(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Label,
    Line,
    BoxType,
    Table,
    Polyline,
    Linefill,
    ChartPoint,
    VolumeRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Int,
    Float,
    Bool,
    String,
    Color,
}
