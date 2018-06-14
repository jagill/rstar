use envelope::Envelope;
use node::{ParentNodeData, RTreeNode};
use num_traits::Bounded;
use object::PointDistance;
use params::RTreeParams;
use point::{min_inline, Point};
use std::collections::binary_heap::BinaryHeap;

struct RTreeNodeDistanceWrapper<'a, T, Params>
where
    T: PointDistance + 'a,
    Params: RTreeParams + 'a,
{
    node: &'a RTreeNode<T, Params>,
    distance: <<T::Envelope as Envelope>::Point as Point>::Scalar,
}

impl<'a, T, Params> PartialEq for RTreeNodeDistanceWrapper<'a, T, Params>
where
    T: PointDistance,
    Params: RTreeParams,
{
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl<'a, T, Params> PartialOrd for RTreeNodeDistanceWrapper<'a, T, Params>
where
    T: PointDistance,
    Params: RTreeParams,
{
    fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
        // Inverse comparison creates a min heap
        other.distance.partial_cmp(&self.distance)
    }
}

impl<'a, T, Params> Eq for RTreeNodeDistanceWrapper<'a, T, Params>
where
    T: PointDistance,
    Params: RTreeParams,
{
}

impl<'a, T, Params> Ord for RTreeNodeDistanceWrapper<'a, T, Params>
where
    T: PointDistance,
    Params: RTreeParams,
{
    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl<'a, 'b, T, Params> NearestNeighborIterator<'a, 'b, T, Params>
where
    T: PointDistance,
    Params: RTreeParams,
{
    pub fn new(
        root: &'a ParentNodeData<T, Params>,
        query_point: &'b <T::Envelope as Envelope>::Point,
    ) -> Self {
        let mut result = NearestNeighborIterator {
            nodes: BinaryHeap::with_capacity(20),
            query_point: query_point,
        };
        result.extend_heap(&root.children);
        result
    }

    fn extend_heap(&mut self, children: &'a [RTreeNode<T, Params>]) {
        let &mut NearestNeighborIterator {
            ref mut nodes,
            ref query_point,
        } = self;
        nodes.extend(children.iter().map(|child| {
            let distance = match child {
                RTreeNode::Parent(ref data) => data.envelope.distance_2(query_point),
                RTreeNode::Leaf(ref t) => t.distance_2(query_point),
            };

            RTreeNodeDistanceWrapper {
                node: child,
                distance: distance,
            }
        }));
    }
}

impl<'a, 'b, T, Params> Iterator for NearestNeighborIterator<'a, 'b, T, Params>
where
    T: PointDistance,
    Params: RTreeParams,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.nodes.pop() {
            match current {
                RTreeNodeDistanceWrapper {
                    node: RTreeNode::Parent(ref data),
                    ..
                } => {
                    self.extend_heap(&data.children);
                }
                RTreeNodeDistanceWrapper {
                    node: RTreeNode::Leaf(ref t),
                    ..
                } => {
                    return Some(t);
                }
            }
        }
        None
    }
}

pub struct NearestNeighborIterator<'a, 'b, T, Params>
where
    T: PointDistance + 'a + 'b,
    Params: RTreeParams + 'a + 'b,
{
    nodes: BinaryHeap<RTreeNodeDistanceWrapper<'a, T, Params>>,
    query_point: &'b <T::Envelope as Envelope>::Point,
}

pub fn nearest_neighbor<'a, 'b, T, Params>(
    node: &'a ParentNodeData<T, Params>,
    query_point: &'b <T::Envelope as Envelope>::Point,
) -> Option<&'a T>
where
    Params: RTreeParams,
    T: PointDistance,
{
    fn extend_heap<'a, 'b, T, Params>(
        nodes: &mut BinaryHeap<RTreeNodeDistanceWrapper<'a, T, Params>>,
        node: &'a ParentNodeData<T, Params>,
        query_point: &'b <T::Envelope as Envelope>::Point,
        min_max_distance: &mut <<T::Envelope as Envelope>::Point as Point>::Scalar,
    ) where
        T: PointDistance + 'a,
        Params: RTreeParams,
    {
        for child in &node.children {
            let distance = match child {
                RTreeNode::Parent(ref data) => data.envelope.distance_2(query_point),
                RTreeNode::Leaf(ref t) => t.distance_2(query_point),
            };
            if &distance <= min_max_distance {
                *min_max_distance = min_inline(
                    *min_max_distance,
                    child.envelope().min_max_dist_2(query_point),
                );
                nodes.push(RTreeNodeDistanceWrapper {
                    node: child,
                    distance: distance,
                });
            }
        }
    }

    // Calculate smallest minmax-distance
    let mut smallest_min_max: <<T::Envelope as Envelope>::Point as Point>::Scalar =
        Bounded::max_value();
    let mut nodes = BinaryHeap::with_capacity(20);
    extend_heap(&mut nodes, node, query_point, &mut smallest_min_max);
    while let Some(current) = nodes.pop() {
        match current {
            RTreeNodeDistanceWrapper {
                node: RTreeNode::Parent(ref data),
                ..
            } => {
                extend_heap(&mut nodes, data, query_point, &mut smallest_min_max);
            }
            RTreeNodeDistanceWrapper {
                node: RTreeNode::Leaf(ref t),
                ..
            } => {
                return Some(t);
            }
        }
    }
    None
}

#[cfg(test)]
mod test {
    use object::PointDistance;
    use rtree::RTree;
    use testutils::create_random_points;

    #[test]
    fn test_nearest_neighbor_empty() {
        let tree: RTree<[f32; 2]> = RTree::new();
        assert!(tree.nearest_neighbor(&[0.0, 213.0]).is_none());
    }

    #[test]
    fn test_nearest_neighbor() {
        let points = create_random_points(1000, *b"syst3mAtisatioNs");
        let mut tree = RTree::new();
        for p in &points {
            tree.insert(*p);
        }
        let sample_points = create_random_points(100, *b"wholEh3artednE5s");
        for sample_point in &sample_points {
            let mut nearest = None;
            let mut closest_dist = ::std::f64::INFINITY;
            for point in &points {
                let delta = [point[0] - sample_point[0], point[1] - sample_point[1]];
                let new_dist = delta[0] * delta[0] + delta[1] * delta[1];
                if new_dist < closest_dist {
                    closest_dist = new_dist;
                    nearest = Some(point);
                }
            }
            assert_eq!(nearest, tree.nearest_neighbor(sample_point));
        }
    }

    #[test]
    fn test_nearest_neighbor_iterator() {
        let mut points = create_random_points(1000, *b"pseudo4gGressive");
        let mut tree = RTree::new();
        for p in &points {
            tree.insert(*p);
        }

        let sample_points = create_random_points(50, *b"1ntraMolecularly");
        for sample_point in sample_points {
            points.sort_by(|r, l| {
                r.distance_2(&sample_point)
                    .partial_cmp(&l.distance_2(&sample_point))
                    .unwrap()
            });
            let collected: Vec<_> = tree.nearest_neighbor_iter(&sample_point).cloned().collect();
            assert_eq!(points, collected);
        }
    }
}