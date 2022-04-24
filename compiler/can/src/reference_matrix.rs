// see if we get better performance with different integer types
pub(crate) type Element = usize;
pub(crate) type BitVec = bitvec::vec::BitVec<Element>;
pub(crate) type BitSlice = bitvec::prelude::BitSlice<Element>;

/// A square boolean matrix used to store relations
///
/// We use this for sorting definitions so every definition is defined before it is used.
/// This functionality is also used to spot and report invalid recursion.
#[derive(Debug)]
pub(crate) struct ReferenceMatrix {
    bitvec: BitVec,
    length: usize,
}

impl ReferenceMatrix {
    pub fn new(length: usize) -> Self {
        Self {
            bitvec: BitVec::repeat(false, length * length),
            length,
        }
    }

    pub fn references_for(&self, row: usize) -> impl Iterator<Item = usize> + '_ {
        self.row_slice(row).iter_ones()
    }

    #[inline(always)]
    fn row_slice(&self, row: usize) -> &BitSlice {
        &self.bitvec[row * self.length..][..self.length]
    }

    #[inline(always)]
    pub fn set_row_col(&mut self, row: usize, col: usize, value: bool) {
        self.bitvec.set(row * self.length + col, value)
    }

    #[inline(always)]
    pub fn get(&self, index: usize) -> bool {
        self.bitvec[index]
    }

    pub fn is_recursive(&self, index: usize) -> bool {
        let mut scheduled = self.row_slice(index).to_bitvec();
        let mut visited = self.row_slice(index).to_bitvec();

        // yes this is a bit inefficient because rows are visited repeatedly.
        while scheduled.any() {
            for one in scheduled.iter_ones() {
                if one == index {
                    return true;
                }

                visited |= self.row_slice(one)
            }

            // i.e. visited did not change
            if visited.count_ones() == scheduled.count_ones() {
                break;
            }

            scheduled |= &visited;
        }

        false
    }
}

// Topological sort and strongly-connected components
//
// Adapted from the Pathfinding crate v2.0.3 by Samuel Tardieu <sam@rfc1149.net>,
// licensed under the Apache License, version 2.0 - https://www.apache.org/licenses/LICENSE-2.0
//
// The original source code can be found at: https://github.com/samueltardieu/pathfinding
//
// Thank you, Samuel!
impl ReferenceMatrix {
    pub fn topological_sort_into_groups(&self) -> TopologicalSort {
        if self.length == 0 {
            return TopologicalSort::Groups { groups: Vec::new() };
        }

        let mut preds_map: Vec<i64> = vec![0; self.length];

        // this is basically summing the columns, I don't see a better way to do it
        for row in self.bitvec.chunks(self.length) {
            for succ in row.iter_ones() {
                preds_map[succ] += 1;
            }
        }

        let mut groups = Vec::<Vec<u32>>::new();

        // the initial group contains all symbols with no predecessors
        let mut prev_group: Vec<u32> = preds_map
            .iter()
            .enumerate()
            .filter_map(|(node, &num_preds)| {
                if num_preds == 0 {
                    Some(node as u32)
                } else {
                    None
                }
            })
            .collect();

        if prev_group.is_empty() {
            let remaining: Vec<u32> = (0u32..self.length as u32).collect();

            return TopologicalSort::HasCycles {
                groups: Vec::new(),
                nodes_in_cycle: remaining,
            };
        }

        while preds_map.iter().any(|x| *x > 0) {
            let mut next_group = Vec::<u32>::new();
            for node in &prev_group {
                for succ in self.references_for(*node as usize) {
                    {
                        let num_preds = preds_map.get_mut(succ).unwrap();
                        *num_preds = num_preds.saturating_sub(1);
                        if *num_preds > 0 {
                            continue;
                        }
                    }

                    // NOTE: we use -1 to mark nodes that have no predecessors, but are already
                    // part of an earlier group. That ensures nodes are added to just 1 group
                    let count = preds_map[succ];
                    preds_map[succ] = -1;

                    if count > -1 {
                        next_group.push(succ as u32);
                    }
                }
            }
            groups.push(std::mem::replace(&mut prev_group, next_group));
            if prev_group.is_empty() {
                let remaining: Vec<u32> = (0u32..self.length as u32)
                    .filter(|i| preds_map[*i as usize] > 0)
                    .collect();

                return TopologicalSort::HasCycles {
                    groups,
                    nodes_in_cycle: remaining,
                };
            }
        }
        groups.push(prev_group);

        TopologicalSort::Groups { groups }
    }

    /// Get the strongly-connected components of the set of input nodes.
    pub fn strongly_connected_components(&self, nodes: &[u32]) -> Vec<Vec<u32>> {
        let mut params = Params::new(self.length, nodes);

        'outer: loop {
            for (node, value) in params.preorders.iter().enumerate() {
                if let Preorder::Removed = value {
                    continue;
                }

                recurse_onto(self.length, &self.bitvec, node, &mut params);

                continue 'outer;
            }

            break params.scc;
        }
    }
}

pub(crate) enum TopologicalSort {
    /// There were no cycles, all nodes have been partitioned into groups
    Groups { groups: Vec<Vec<u32>> },
    /// Cycles were found. All nodes that are not part of a cycle have been partitioned
    /// into groups. The other elements are in the `cyclic` vector. However, there may be
    /// many cycles, or just one big one. Use strongly-connected components to find out
    /// exactly what the cycles are and how they fit into the groups.
    HasCycles {
        groups: Vec<Vec<u32>>,
        nodes_in_cycle: Vec<u32>,
    },
}

#[derive(Clone, Copy)]
enum Preorder {
    Empty,
    Filled(usize),
    Removed,
}

struct Params {
    preorders: Vec<Preorder>,
    c: usize,
    p: Vec<u32>,
    s: Vec<u32>,
    scc: Vec<Vec<u32>>,
    scca: Vec<u32>,
}

impl Params {
    fn new(length: usize, group: &[u32]) -> Self {
        let mut preorders = vec![Preorder::Removed; length];

        for value in group {
            preorders[*value as usize] = Preorder::Empty;
        }

        Self {
            preorders,
            c: 0,
            s: Vec::new(),
            p: Vec::new(),
            scc: Vec::new(),
            scca: Vec::new(),
        }
    }
}

fn recurse_onto(length: usize, bitvec: &BitVec, v: usize, params: &mut Params) {
    params.preorders[v] = Preorder::Filled(params.c);

    params.c += 1;

    params.s.push(v as u32);
    params.p.push(v as u32);

    for w in bitvec[v * length..][..length].iter_ones() {
        if !params.scca.contains(&(w as u32)) {
            match params.preorders[w] {
                Preorder::Filled(pw) => loop {
                    let index = *params.p.last().unwrap();

                    match params.preorders[index as usize] {
                        Preorder::Empty => unreachable!(),
                        Preorder::Filled(current) => {
                            if current > pw {
                                params.p.pop();
                            } else {
                                break;
                            }
                        }
                        Preorder::Removed => {}
                    }
                },
                Preorder::Empty => recurse_onto(length, bitvec, w, params),
                Preorder::Removed => {}
            }
        }
    }

    if params.p.last() == Some(&(v as u32)) {
        params.p.pop();

        let mut component = Vec::new();
        while let Some(node) = params.s.pop() {
            component.push(node);
            params.scca.push(node);
            params.preorders[node as usize] = Preorder::Removed;
            if node as usize == v {
                break;
            }
        }
        params.scc.push(component);
    }
}