//! The intermediate representaton used for the conversion between a factorio blue


/// An entity in the intermerdiate representation can either be a splitter or a merger.
/// 
/// A splitter takes a single input and splits it in two, and optionally prioritizes
/// an output over the other.
/// 
/// A merger takes two inputs, optionally prioritizing one, and combines them into
/// a single output.
/// 
/// A belt is represented by two different sides.
/// 
/// # Examples
/// 
/// ## Belt side junction
/// 
/// ```
/// ⇉⇉⇉⇉⇉⇉
///    ⇈
///    ⇈
///    ⇈
///    
/// ```
/// 
/// A vertical belt joins an horizontal one from the side. The two sides
/// of the vertical one are merged, with priority given to the left one.
/// Then this combination is merged with the right side of the horizontal one, with
/// priority given to the horizontal belt.
/// 
///
pub enum Entity {
    Splitter(Splitter),
    Merger(Merger),
}

pub struct Merger {

}

pub struct Splitter {

}

pub trait NameMe {
    
}