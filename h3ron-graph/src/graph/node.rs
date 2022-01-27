use std::ops::{Add, AddAssign};

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Origin,
    Destination,
    OriginAndDestination,
}

impl NodeType {
    pub const fn is_origin(&self) -> bool {
        match self {
            NodeType::Origin => true,
            NodeType::Destination => false,
            NodeType::OriginAndDestination => true,
        }
    }

    pub const fn is_destination(&self) -> bool {
        match self {
            NodeType::Origin => false,
            NodeType::Destination => true,
            NodeType::OriginAndDestination => true,
        }
    }
}

impl Add<Self> for NodeType {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if rhs == self {
            self
        } else {
            Self::OriginAndDestination
        }
    }
}

impl AddAssign<Self> for NodeType {
    fn add_assign(&mut self, rhs: Self) {
        if self != &rhs {
            *self = Self::OriginAndDestination
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::node::NodeType;

    #[test]
    fn test_nodetype_add() {
        assert_eq!(NodeType::Origin, NodeType::Origin + NodeType::Origin);
        assert_eq!(
            NodeType::Destination,
            NodeType::Destination + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::Origin + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::OriginAndDestination + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::Destination + NodeType::Origin
        );
    }

    #[test]
    fn test_nodetype_addassign() {
        let mut n1 = NodeType::Origin;
        n1 += NodeType::Origin;
        assert_eq!(n1, NodeType::Origin);

        let mut n2 = NodeType::Origin;
        n2 += NodeType::OriginAndDestination;
        assert_eq!(n2, NodeType::OriginAndDestination);

        let mut n3 = NodeType::Destination;
        n3 += NodeType::OriginAndDestination;
        assert_eq!(n3, NodeType::OriginAndDestination);

        let mut n4 = NodeType::Destination;
        n4 += NodeType::Origin;
        assert_eq!(n4, NodeType::OriginAndDestination);
    }
}
