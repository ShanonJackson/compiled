use super::ast::*;

pub fn stringify_nodes(nodes: &[Node]) -> String {
    let mut result = String::new();
    for node in nodes {
        result.push_str(&stringify_node(node));
    }
    result
}

pub fn stringify_node(node: &Node) -> String {
    match &*node.borrow() {
        NodeData::Word(word) => word.value.clone(),
        NodeData::Space(space) => space.value.clone(),
        NodeData::String(string) => {
            let mut output = String::new();
            if let Some(quote) = string.quote {
                output.push(quote);
                output.push_str(&string.value);
                if !string.unclosed {
                    output.push(quote);
                }
            } else {
                output.push_str(&string.value);
            }
            output
        }
        NodeData::Comment(comment) => {
            let mut output = String::from("/*");
            output.push_str(&comment.value);
            if !comment.unclosed {
                output.push_str("*/");
            }
            output
        }
        NodeData::Div(div) => {
            let mut output = String::new();
            output.push_str(&div.before);
            output.push_str(&div.value);
            output.push_str(&div.after);
            output
        }
        NodeData::Function(function) => {
            let mut output = String::new();
            output.push_str(&function.value);
            output.push('(');
            output.push_str(&function.before);
            output.push_str(&stringify_nodes(&function.nodes));
            output.push_str(&function.after);
            if !function.unclosed {
                output.push(')');
            }
            output
        }
        NodeData::UnicodeRange(range) => range.value.clone(),
    }
}
