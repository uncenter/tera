use std::collections::{HashMap, LinkedList};

use pest::prelude::*;
use errors::Result;


#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    List(LinkedList<Node>),

    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),

    Math {lhs: Box<Node>, rhs: Box<Node>, operator: String},
    Logic {lhs: Box<Node>, rhs: Box<Node>, operator: String},
    Not(Box<Node>),

    If {condition_nodes: LinkedList<Node>, else_node: Option<Box<Node>>},
    // represents if/elif. condition (Bool, Math, Logic, Test), body (a List)
    Conditional {condition: Box<Node>, body: Box<Node>},

    For {variable: String, array: String, body: Box<Node>},
    Block {name: String, body: Box<Node>},
    Super,

    // params is the list of the params names
    Macro {name: String, params: LinkedList<String>, body: Box<Node>},
    // import looks like `{% import "macros.html" as macros %}`
    // tpl_name refers to "macros.html" and name to macros in that example
    ImportMacro {tpl_name: String, name: String},
    // Macros are called like `{{ my_macros::macro1(foo=1, bar=bar) }}`
    // params are kwargs {name: expression}
    MacroCall {namespace: String, name: String, params: HashMap<String, Node>},

    // params are expressions
    Test {expression: Box<Node>, name: String, params: LinkedList<Node>},

    // params are expressions
    Filter {name: String, params: HashMap<String, Node>},
    Identifier {name: String, filters: Option<LinkedList<Node>>},

    Raw(String),
    Extends(String),
    VariableBlock(Box<Node>),
    Include(String),
}

impl Node {
    pub fn get_children(&self) -> LinkedList<Node> {
        match *self {
            Node::List(ref l) => l.clone(),
            Node::If {ref condition_nodes, ..} => condition_nodes.clone(),
            _ => panic!("tried to get_children on a non-list/if node")
        }
    }
}

impl_rdp! {
    grammar! {
        whitespace = _{ ([" "] | ["\t"] | ["\r"] | ["\n"])+ }

        // basic blocks of the language
        op_or        = { ["or"] }
        op_wrong_or  = { ["||"] }
        op_and       = { ["and"] }
        op_wrong_and = { ["&&"] }
        op_not       = { ["not"] }
        op_lte       = { ["<="] }
        op_gte       = { [">="] }
        op_lt        = { ["<"] }
        op_gt        = { [">"] }
        op_eq        = { ["=="] }
        op_ineq      = { ["!="] }
        op_plus      = { ["+"] }
        op_minus     = { ["-"] }
        op_times     = { ["*"] }
        op_slash     = { ["/"] }
        op_true      = { ["true"] }
        op_false     = { ["false"] }
        boolean      = _{ op_true | op_false }
        op_filter    = _{ ["|"] }

        int   = @{ ["-"]? ~ (["0"] | ['1'..'9'] ~ ['0'..'9']*) }
        float = @{
            ["-"]? ~
                ["0"] ~ ["."] ~ ['0'..'9']+ |
                ['1'..'9'] ~ ['0'..'9']* ~ ["."] ~ ['0'..'9']+
        }
        // matches anything between 2 double quotes
        string  = @{ ["\""] ~ (!(["\""]) ~ any )* ~ ["\""]}

        // FUNCTIONS
        // Almost same as identifier minus no . allowed, used everywhere other
        // than accessing context variables
        simple_ident = @{
            (['a'..'z'] | ['A'..'Z'] | ["_"]) ~
            (['a'..'z'] | ['A'..'Z'] | ["_"] | ['0'..'9'])*
        }

        // named args
        fn_arg  = @{ simple_ident ~ ["="] ~ expression}
        fn_args = !@{ fn_arg ~ ([","] ~ fn_arg )* }
        fn_call = { simple_ident ~ ["("] ~ fn_args ~ [")"] | simple_ident }

        filters = { (op_filter ~ fn_call)+ }

        identifier = @{
            (['a'..'z'] | ['A'..'Z'] | ["_"]) ~
            (['a'..'z'] | ['A'..'Z'] | ["_"] | ["."] | ['0'..'9'])*
        }
        identifier_with_filter = { identifier ~ filters }
        idents = _{ identifier_with_filter | identifier }

        // macros
        // TODO: add default arg?
        macro_param = @{ simple_ident }
        macro_params = !@{ macro_param ~ ([","] ~ macro_param )* }
        macro_definition = _{ identifier ~ ["("] ~ macro_params? ~ [")"]}
        macro_call = { simple_ident ~ ["::"] ~ simple_ident ~ ["("] ~ fn_args? ~ [")"] }

        // Variable tests.
        test_fn_param = { expression }
        test_fn_params = {
            test_fn_param
            | (["("] ~ test_fn_param ~ ([","] ~ test_fn_param)* ~ [")"])
        }
        test_fn = !@{ simple_ident ~ test_fn_params? }
        test = { ["is"] ~ test_fn }

        // Precedence climbing
        expression = _{
            // boolean first so they are not caught as identifiers
            { boolean | string | idents | float | int }
            comparison  = { op_gt | op_lt | op_eq | op_ineq | op_lte | op_gte }
            add_sub     = { op_plus | op_minus }
            mul_div     = { op_times | op_slash }
        }

        logic_expression = _{
            { op_not? ~ expression }
            or          = { op_or | op_wrong_or }
            and         = { op_and | op_wrong_and }
        }

        // Tera specific things

        // different types of blocks
        variable_start = _{ ["{{"] }
        variable_end   = _{ ["}}"] }
        tag_start      = _{ ["{%"] }
        tag_end        = _{ ["%}"] }
        comment_start  = _{ ["{#"] }
        comment_end    = _{ ["#}"] }
        block_start    = _{ variable_start | tag_start | comment_start }

        // Actual tags
        include_tag      = !@{ tag_start ~ ["include"] ~ string ~ tag_end }
        import_macro_tag = !@{ tag_start ~ ["import"] ~ string ~ ["as"] ~ simple_ident ~ tag_end}
        extends_tag      = !@{ tag_start ~ ["extends"] ~ string ~ tag_end }
        variable_tag     = !@{ variable_start ~ (macro_call | logic_expression) ~ variable_end }
        super_tag        = !@{ variable_start ~ ["super()"] ~ variable_end }
        comment_tag      = !@{ comment_start ~ (!comment_end ~ any )* ~ comment_end }
        block_tag        = !@{ tag_start ~ ["block"] ~ identifier ~ tag_end }
        macro_tag        = !@{ tag_start ~ ["macro"] ~ macro_definition ~ tag_end }
        if_tag           = !@{ tag_start ~ ["if"] ~ logic_expression ~ test? ~ tag_end }
        elif_tag         = !@{ tag_start ~ ["elif"] ~ logic_expression ~ test? ~ tag_end }
        else_tag         = !@{ tag_start ~ ["else"] ~ tag_end }
        for_tag          = !@{ tag_start ~ ["for"] ~ identifier ~ ["in"] ~ idents ~ tag_end }
        raw_tag          = !@{ tag_start ~ ["raw"] ~ tag_end }
        endraw_tag       = !@{ tag_start ~ ["endraw"] ~ tag_end }
        endblock_tag     = !@{ tag_start ~ ["endblock"] ~ identifier ~ tag_end }
        endmacro_tag     = !@{ tag_start ~ ["endmacro"] ~ identifier ~ tag_end }
        endif_tag        = !@{ tag_start ~ ["endif"] ~ tag_end }
        endfor_tag       = !@{ tag_start ~ ["endfor"] ~ tag_end }

        elif_block = { elif_tag ~ content* }
        raw_text   = { (!endraw_tag ~ any )* }
        text       = { (!(block_start) ~ any )+ }

        // smaller sets of allowed content in macros
        macro_content = @{
            include_tag |
            variable_tag |
            comment_tag |
            if_tag ~ macro_content* ~ elif_block* ~ (else_tag ~ macro_content*)? ~ endif_tag |
            for_tag ~ macro_content* ~ endfor_tag |
            raw_tag ~ raw_text ~ endraw_tag |
            text
        }

        // smaller set of allowed content in block
        // currently identical as `macro_content` but will change when super() is added
        block_content = @{
            include_tag |
            super_tag |
            variable_tag |
            comment_tag |
            block_tag ~ block_content* ~ endblock_tag |
            if_tag ~ block_content* ~ elif_block* ~ (else_tag ~ block_content*)? ~ endif_tag |
            for_tag ~ block_content* ~ endfor_tag |
            raw_tag ~ raw_text ~ endraw_tag |
            text
        }

        content = @{
            include_tag |
            import_macro_tag |
            variable_tag |
            comment_tag |
            macro_tag ~ macro_content* ~ endmacro_tag |
            block_tag ~ block_content* ~ endblock_tag |
            if_tag ~ content* ~ elif_block* ~ (else_tag ~ content*)? ~ endif_tag |
            for_tag ~ content* ~ endfor_tag |
            raw_tag ~ raw_text ~ endraw_tag |
            text
        }

        // top level rule
        template = @{ soi ~ extends_tag? ~ content* ~ eoi }
    }

    process! {
        main(&self) -> Result<Node> {
            (_: template, tpl: _template()) => {
                match tpl {
                    Ok(t) => Ok(Node::List(t)),
                    Err(e) => Err(e)
                }
            }
        }

        _template(&self) -> Result<LinkedList<Node>> {
            (_: extends_tag, &name: string, tail: _template()) => {
                let mut tail2 = tail?;
                tail2.push_front(Node::Extends(name.replace("\"", "").to_string()));
                Ok(tail2)
            },
            (_: extends_tag, &name: string) => {
                let mut body = LinkedList::new();
                body.push_front(Node::Extends(name.replace("\"", "").to_string()));
                Ok(body)
            },
            (_: content, node: _content(), tail: _template()) => {
                let mut tail2 = tail?;
                match node? {
                    Some(n) => { tail2.push_front(n); }
                    None => ()
                };
                Ok(tail2)
            },
            (_: macro_content, node: _content(), tail: _template()) => {
                let mut tail2 = tail?;
                match node? {
                    Some(n) => { tail2.push_front(n); }
                    None => ()
                };
                Ok(tail2)
            },
            (_: block_content, node: _content(), tail: _template()) => {
                let mut tail2 = tail?;
                match node? {
                    Some(n) => { tail2.push_front(n); }
                    None => ()
                };
                Ok(tail2)
            },
            () => Ok(LinkedList::new())
        }

        // Option since we don't want comments in the AST
        _content(&self) -> Result<Option<Node>> {
            (&head: text) => {
                Ok(Some(Node::Text(head.to_string())))
            },
            (_: include_tag, &name: string) => {
                Ok(Some(Node::Include(name.trim_matches('"').to_string())))
            },
            (_: import_macro_tag, &tpl_name: string, &name: simple_ident) => {
                Ok(Some(Node::ImportMacro {
                    tpl_name: tpl_name.trim_matches('"').to_string(),
                    name: name.to_string(),
                }))
            },
            (_: variable_tag, _: macro_call, &namespace: simple_ident, &name: simple_ident, params: _fn_args()) => {
                Ok(Some(Node::MacroCall {
                    namespace: namespace.to_string(),
                    name: name.to_string(),
                    params: params?
                }))
            },
            (_: variable_tag, exp: _expression()) => {
                Ok(Some(Node::VariableBlock(Box::new(exp?))))
            },
            (_: raw_tag, &body: raw_text, _: endraw_tag) => {
                Ok(Some(Node::Raw(body.to_string())))
            },
            (_: block_tag, &name: identifier, body: _template(), _: endblock_tag, &end_name: identifier) => {
                if name != end_name {
                    let (line_no, col_no) = self.input().line_col(self.input.pos());
                    bail!(
                        "Block `{}` is closing at line {}, col {} but we were expecting `{}` to be closing",
                        end_name, line_no, col_no, name
                    );
                }
                Ok(Some(Node::Block {
                    name: name.to_string(),
                    body: Box::new(Node::List(body?))
                }))
            },
            (_: macro_tag, &name: identifier, params: _macro_def_params(), body: _template(), _: endmacro_tag, &end_name: identifier) => {
                if name != end_name {
                    let (line_no, col_no) = self.input().line_col(self.input.pos());
                    bail!(
                        "Macro `{}` is closing at line {}, col {} but we were expecting `{}` to be closing",
                        end_name, line_no, col_no, name
                    );
                }
                Ok(Some(Node::Macro {
                    name: name.to_string(),
                    params: params,
                    body: Box::new(Node::List(body?))
                }))
            },
            (_: for_tag, &variable: identifier, &array: identifier, body: _template(), _: endfor_tag) => {
                Ok(Some(Node::For {
                    variable: variable.to_string(),
                    array: array.to_string(),
                    body: Box::new(Node::List(body?))
                }))
            },
            // only if
            (_: if_tag, cond: _condition(), body: _template(), _: endif_tag) => {
                let mut condition_nodes = LinkedList::new();
                condition_nodes.push_front(Node::Conditional {
                    condition: Box::new(cond?),
                    body: Box::new(Node::List(body?)),
                });

                Ok(Some(Node::If {
                    condition_nodes: condition_nodes,
                    else_node: None,
                }))
            },
            // if/elifs/else
            (_: if_tag, cond: _condition(), body: _template(), elifs: _elifs(), _: else_tag, else_body: _template(), _: endif_tag) => {
                let mut condition_nodes = LinkedList::new();
                condition_nodes.push_front(Node::Conditional {
                    condition: Box::new(cond?),
                    body: Box::new(Node::List(body?)),
                });

                for elif in elifs? {
                    condition_nodes.push_back(elif)
                }

                Ok(Some(Node::If {
                    condition_nodes: condition_nodes,
                    else_node: Some(Box::new(Node::List(else_body?))),
                }))
            },
            // if/elifs
            (_: if_tag, cond: _condition(), body: _template(), elifs: _elifs(), _: endif_tag) => {
                let mut condition_nodes = LinkedList::new();
                condition_nodes.push_front(Node::Conditional {
                    condition: Box::new(cond?),
                    body: Box::new(Node::List(body?)),
                });

                for elif in elifs? {
                    condition_nodes.push_back(elif)
                }

                Ok(Some(Node::If {
                    condition_nodes: condition_nodes,
                    else_node: None,
                }))
            },
            // if/else
            (_: if_tag, cond: _condition(), body: _template(), _: else_tag, else_body: _template(), _: endif_tag) => {
                let mut condition_nodes = LinkedList::new();
                condition_nodes.push_front(Node::Conditional {
                    condition: Box::new(cond?),
                    body: Box::new(Node::List(body?)),
                });

                Ok(Some(Node::If {
                    condition_nodes: condition_nodes,
                    else_node: Some(Box::new(Node::List(else_body?))),
                }))
            },
            (_: super_tag) => {
                Ok(Some(Node::Super))
            },
            (_: comment_tag) => {
                Ok(None)
            }
        }

        _condition(&self) -> Result<Node> {
            // Expression with a test.
            (exp: _expression(), _: test, test_args: _test()) => {
                let (name, params) = test_args?;
                Ok(Node::Test {
                    expression: Box::new(exp?),
                    name: name,
                    params: params,
                })
            },
            // Expression without a test.
            (exp: _expression()) => {
                exp
            }
        }

        _elifs(&self) -> Result<LinkedList<Node>> {
            (_: elif_block, node: _if(), tail: _elifs()) => {
                let mut tail2 = tail?;
                tail2.push_front(node?);
                Ok(tail2)
            },
            () => Ok(LinkedList::new())
        }

        _if(&self) -> Result<Node> {
            (_: if_tag, cond: _condition(), body: _template()) => {
                Ok(Node::Conditional {
                    condition: Box::new(cond?),
                    body: Box::new(Node::List(body?)),
                })
            },
            (_: elif_tag, cond: _condition(), body: _template()) => {
                Ok(Node::Conditional {
                    condition: Box::new(cond?),
                    body: Box::new(Node::List(body?)),
                })
            },
        }

        _fn_args(&self) -> Result<HashMap<String, Node>> {
             // first arg of the fn
            (_: fn_args, _: fn_arg, &name: simple_ident, exp: _expression(), tail: _fn_args()) => {
                let mut tail2 = tail?;
                tail2.insert(name.to_string(), exp?);
                Ok(tail2)
            },
            // arguments after the first
            (_: fn_arg, &name: simple_ident, exp: _expression(), tail: _fn_args()) => {
                let mut tail2 = tail?;
                tail2.insert(name.to_string(), exp?);
                Ok(tail2)
            },
            () => Ok(HashMap::new())
        }

        _fn(&self) -> Result<Node> {
            (_: fn_call, &name: simple_ident, args: _fn_args()) => {
                Ok(Node::Filter{name: name.to_string(), params: args?})
            },
            // The filters parser will need to consume the `fn_call` token
            // It might not be needed in next version of pest
            // https://github.com/dragostis/pest/issues/74
            (&name: simple_ident, args: _fn_args()) => {
                Ok(Node::Filter{name: name.to_string(), params: args?})
            },
        }

        _filters(&self) -> Result<LinkedList<Node>> {
            (_: filters, filter: _fn(), tail: _filters()) => {
                let mut tail2 = tail?;
                tail2.push_front(filter?);
                Ok(tail2)
            },
            (_: fn_call, filter: _fn(), tail: _filters()) => {
                let mut tail2 = tail?;
                tail2.push_front(filter?);
                Ok(tail2)
            },
            () => Ok(LinkedList::new())
        }

        _macro_def_params(&self) -> LinkedList<String> {
            // first arg of many
            (_: macro_params, _: macro_param, &name: simple_ident, mut tail: _macro_def_params()) => {
                tail.push_front(name.to_string());
                tail
            },
            // arguments after the first of many
            (_: macro_param, &name: simple_ident, mut tail: _macro_def_params()) => {
                tail.push_front(name.to_string());
                tail
            },
            // Base case
            () => LinkedList::new()
        }

        _test_fn_params(&self) -> (Result<LinkedList<Node>>) {
            // first arg of many
            (_: test_fn_params, _: test_fn_param, value: _expression(), tail: _test_fn_params()) => {
                let mut tail = tail?;
                tail.push_front(value?);
                Ok(tail)
            },
            // arguments after the first of many
            (_: test_fn_param, value: _expression(), tail: _test_fn_params()) => {
                let mut tail = tail?;
                tail.push_front(value?);
                Ok(tail)
            },
            // Base case.
            () => (Ok(LinkedList::new()))
        }

        _test(&self) -> Result<(String, LinkedList<Node>)> {
            (_: test_fn, &name: simple_ident, params: _test_fn_params()) => {
                Ok((name.to_string(), params?))
            },
        }

        _expression(&self) -> Result<Node> {
            (_: add_sub, left: _expression(), sign, right: _expression()) => {
                Ok(Node::Math {
                    lhs: Box::new(left?),
                    rhs: Box::new(right?),
                    operator: match sign.rule {
                        Rule::op_plus => "+".to_string(),
                        Rule::op_minus => "-".to_string(),
                        _ => unreachable!()
                    }
                })
            },
            (_: mul_div, left: _expression(), sign, right: _expression()) => {
                Ok(Node::Math {
                    lhs: Box::new(left?),
                    rhs: Box::new(right?),
                    operator: match sign.rule {
                        Rule::op_times => "*".to_string(),
                        Rule::op_slash => "/".to_string(),
                        _ => unreachable!()
                    }
                })
            },
            (_: comparison, left: _expression(), sign, right: _expression()) => {
                Ok(Node::Logic {
                    lhs: Box::new(left?),
                    rhs: Box::new(right?),
                    operator: match sign.rule {
                        Rule::op_gt => ">".to_string(),
                        Rule::op_lt => "<".to_string(),
                        Rule::op_eq => "==".to_string(),
                        Rule::op_ineq => "!=".to_string(),
                        Rule::op_lte => "<=".to_string(),
                        Rule::op_gte => ">=".to_string(),
                        _ => unreachable!()
                    }
                })
            },
            (_: and, left: _expression(), _, right: _expression()) => {
                Ok(Node::Logic {
                    lhs: Box::new(left?),
                    rhs: Box::new(right?),
                    operator: "and".to_string()
                })
            },
            (_: or, left: _expression(), _, right: _expression()) => {
                println!("{:?}", left);
                println!("{:?}", right);
                Ok(Node::Logic {
                    lhs: Box::new(left?),
                    rhs: Box::new(right?),
                    operator: "or".to_string()
                })
            },
            (_: identifier_with_filter, &ident: identifier, tail: _filters()) => {
                Ok(Node::Identifier {
                    name: ident.to_string(),
                    filters: Some(tail?),
                })
            },
            // single not used {% if not admin %} => equivalent to {% if admin == false %}
            (_: op_not, exp: _expression()) => {
                Ok(Node::Not(Box::new(exp?)))
            },
            (&ident: identifier) => {
                Ok(Node::Identifier {name: ident.to_string(), filters: None })
            },
            (&number: int) => {
                Ok(Node::Int(number.parse::<i64>().unwrap()))
            },
            (&number: float) => {
                Ok(Node::Float(number.parse::<f64>().unwrap()))
            },
            (_: op_true) => {
                Ok(Node::Bool(true))
            },
            (_: op_false) => {
                Ok(Node::Bool(false))
            },
            (&text: text) => {
                Ok(Node::Text(text.to_string()))
            },
            (&string: string) => {
                Ok(Node::Text(string.replace("\"", "").to_string()))
            }
        }
    }
}

// We need a little bit of post-processing to
pub fn parse(input: &str) -> Result<Node> {
    let mut parser = Rdp::new(StringInput::new(input));

    if !parser.template() {
        let (_, pos) = parser.expected();
        let (line_no, col_no) = parser.input().line_col(pos);
        bail!("Invalid Tera syntax at line {}, column {}", line_no, col_no);
    }

    // We need to check for deprecated syntaxes
    for token in parser.queue() {
        match token.rule {
            Rule::op_wrong_and => {
                let (line_no, col_no) = parser.input().line_col(token.start);
                bail!("Use `and` instead of `&&` at line {}, column {}", line_no, col_no);
            },
            Rule::op_wrong_or => {
                let (line_no, col_no) = parser.input().line_col(token.start);
                bail!("Use `or` instead of `||` at line {}, column {}", line_no, col_no);
            },
            _ => ()
        };
    }

    parser.main()
}

#[cfg(test)]
mod tests {
    use std::collections::{LinkedList, HashMap};

    use pest::prelude::*;

    use super::{Rdp, Node, parse};

    #[test]
    fn test_int() {
        let mut parser = Rdp::new(StringInput::new("123"));
        assert!(parser.int());
        assert!(parser.end());
    }

    #[test]
    fn test_float() {
        let mut parser = Rdp::new(StringInput::new("123.5"));
        assert!(parser.float());
        assert!(parser.end());
    }

    #[test]
    fn test_identifier() {
        let mut parser = Rdp::new(StringInput::new("client.phone_number"));
        assert!(parser.identifier());
        assert!(parser.end());
    }

    #[test]
    fn test_identifier_with_filter() {
        let mut parser = Rdp::new(
            StringInput::new("phone_number | phone(format=user.country) | truncate(limit=50)")
        );
        assert!(parser.identifier_with_filter());
        assert!(parser.end());
    }

    #[test]
    fn test_text() {
        let mut parser = Rdp::new(StringInput::new("Hello\n 世界"));
        assert!(parser.text());
        assert!(parser.end());
    }

    #[test]
    fn test_text_with_trailing_space() {
        let mut parser = Rdp::new(StringInput::new("Hello\n 世界  "));
        // the text rule itself is not going to parse the trailing space
        // correctly so we are using template here
        assert!(parser.template());
        assert!(parser.end());
    }

    #[test]
    fn test_text_with_leading_space() {
        let mut parser = Rdp::new(StringInput::new("   Hello\n 世界"));
        assert!(parser.text());
        assert!(parser.end());
    }

    #[test]
    fn test_string() {
        let mut parser = Rdp::new(StringInput::new("\"Blabla\""));
        assert!(parser.string());
        assert!(parser.end());
    }

    #[test]
    fn test_extends_tag() {
        let mut parser = Rdp::new(StringInput::new("{% extends \"base.html\" %}"));
        assert!(parser.extends_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_include_tag() {
        let mut parser = Rdp::new(StringInput::new("{% include \"component.html\" %}"));
        assert!(parser.include_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_comment_tag() {
        let mut parser = Rdp::new(StringInput::new("{# some text {{}} #}"));
        assert!(parser.comment_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_block_tag() {
        let mut parser = Rdp::new(StringInput::new("{% block hello %}"));
        assert!(parser.block_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_endblock_tag() {
        let mut parser = Rdp::new(StringInput::new("{% endblock hello %}"));
        assert!(parser.endblock_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_for_tag() {
        let mut parser = Rdp::new(StringInput::new("{% for client in clients %}"));
        assert!(parser.for_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_for_tag_with_filter() {
        let mut parser = Rdp::new(StringInput::new("{% for client in clients | slice(start=1, end=9) %}"));
        assert!(parser.for_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_endfor_tag() {
        let mut parser = Rdp::new(StringInput::new("{% endfor %}"));
        assert!(parser.endfor_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_expression_math() {
        let mut parser = Rdp::new(StringInput::new("1 + 2 + 3 * 9/2 + 2"));
        assert!(parser.logic_expression());
        assert!(parser.end());
    }

    #[test]
    fn test_expression_identifier_logic_simple() {
        let mut parser = Rdp::new(StringInput::new("index + 1 > 1"));
        assert!(parser.logic_expression());
        assert!(parser.end());
    }

    #[test]
    fn test_expression_identifier_logic_complex() {
        let mut parser = Rdp::new(StringInput::new("1 > 2 or 3 == 4 and admin"));
        assert!(parser.logic_expression());
        assert!(parser.end());
    }

    #[test]
    fn test_logic_expression_not_simple() {
        let mut parser = Rdp::new(StringInput::new("not admin"));
        assert!(parser.logic_expression());
        println!("{:?}", parser.queue_with_captures());
        assert!(parser.end());
    }

    #[test]
    fn test_logic_expression_not_expression() {
        let mut parser = Rdp::new(StringInput::new("not user_count or true"));
        assert!(parser.logic_expression());
        assert!(parser.end());
    }

    #[test]
    fn test_if_tag() {
        let mut parser = Rdp::new(StringInput::new("{% if true or show == false %}"));
        assert!(parser.if_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_if_tag_with_filter() {
        let mut parser = Rdp::new(StringInput::new("{% if 1 + something | test %}"));
        assert!(parser.if_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_if_tag_with_test() {
        let mut parser = Rdp::new(StringInput::new("{% if value is defined %}"));
        assert!(parser.if_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_elif_tag_with_test() {
        let mut parser = Rdp::new(StringInput::new("{% elif value is defined %}"));
        assert!(parser.elif_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_variable_tag() {
        let mut parser = Rdp::new(StringInput::new("{{loop.index + 1}}"));
        assert!(parser.variable_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_variable_tag_with_filter() {
        let mut parser = Rdp::new(StringInput::new("{{ greeting | i18n(lang=user.lang) | truncate(limit=50) }}"));
        assert!(parser.variable_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_variable_tag_with_filter_and_string_arg() {
        let mut parser = Rdp::new(StringInput::new("{{ greeting | i18n(lang=\"fr\") }}"));
        assert!(parser.variable_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_variable_tag_macro_call() {
        let mut parser = Rdp::new(StringInput::new("{{ my_macros::macro1(hello=\"world\", foo=bar, hey=1+2) }}"));
        assert!(parser.variable_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_content() {
        let mut parser = Rdp::new(StringInput::new("{% if i18n %}世界{% else %}world{% endif %}"));
        assert!(parser.content());
        assert!(parser.end());
    }

    #[test]
    fn test_macro_tag_no_args() {
        let mut parser = Rdp::new(StringInput::new("{% macro hello_world() %}"));
        assert!(parser.macro_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_macro_tag_with_args() {
        let mut parser = Rdp::new(StringInput::new("{% macro hello_world(greeting, capitalize) %}"));
        assert!(parser.macro_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_endmacro() {
        let mut parser = Rdp::new(StringInput::new("{% endmacro hello_world %}"));
        assert!(parser.endmacro_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_import_macro() {
        let mut parser = Rdp::new(StringInput::new("{% import \"macros.html\" as macros %}"));
        assert!(parser.import_macro_tag());
        assert!(parser.end());
    }

    #[test]
    fn test_template() {
        let mut parser = Rdp::new(StringInput::new("
            {# Greeter template #}
            Hello {% if i18n %}世界{% else %}world{% endif %}
            {% for country in countries %}
                {{ loop.index }}.{{ country }}
            {% endfor %}
        "));
        assert!(parser.template());
        assert!(parser.end());
    }

    #[test]
    fn test_invalid_syntax() {
        let parsed_ast = parse("{% block hey ");
        assert!(parsed_ast.is_err());
        assert_eq!(
            parsed_ast.err().unwrap().description(),
            "Invalid Tera syntax at line 1, column 1"
        );
    }

    #[test]
    fn test_invalid_extends() {
        let parsed_ast = parse("{% extends \"base.html\" %} {% extends \"base.html\" %}");
        assert!(parsed_ast.is_err());
        assert_eq!(
            parsed_ast.err().unwrap().description(),
            "Invalid Tera syntax at line 1, column 27"
        );
    }

    #[test]
    fn test_ast_basic() {
        let parsed_ast = parse(" Hello {{ count + 1 * 2.5 }} {{ true or false and 1 }}");
        let mut ast = LinkedList::new();
        ast.push_front(Node::VariableBlock(
            Box::new(Node::Logic {
                lhs: Box::new(Node::Bool(true)),
                rhs: Box::new(Node::Logic {
                    lhs: Box::new(Node::Bool(false)),
                    rhs: Box::new(Node::Int(1)),
                    operator: "and".to_string()
                }),
                operator: "or".to_string()
            })
        ));
        ast.push_front(Node::Text(" ".to_string()));
        ast.push_front(Node::VariableBlock(
            Box::new(Node::Math {
                lhs: Box::new(Node::Identifier{name: "count".to_string(), filters: None}),
                rhs: Box::new(Node::Math {
                    lhs: Box::new(Node::Int(1)),
                    rhs: Box::new(Node::Float(2.5)),
                    operator: "*".to_string()
                }),
                operator: "+".to_string()
            })
        ));
        ast.push_front(Node::Text(" Hello ".to_string()));

        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_block() {
        let parsed_ast = parse("{% block content %}Hello{% endblock content %}");
        let mut ast = LinkedList::new();
        let mut inner_content = LinkedList::new();
        inner_content.push_front(Node::Text("Hello".to_string()));
        ast.push_front(Node::Block {
            name: "content".to_string(),
            body: Box::new(Node::List(inner_content))
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_for() {
        let parsed_ast = parse("{% for user in users %}{{user.email}}{% endfor %}");
        let mut ast = LinkedList::new();
        let mut inner_content = LinkedList::new();
        inner_content.push_front(Node::VariableBlock(
            Box::new(Node::Identifier {name: "user.email".to_string(), filters: None})
        ));
        ast.push_front(Node::For {
            variable: "user".to_string(),
            array: "users".to_string(),
            body: Box::new(Node::List(inner_content))
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_extends() {
        let parsed_ast = parse("{% extends \"base.html\" %}");
        let mut ast = LinkedList::new();
        ast.push_front(Node::Extends("base.html".to_string()));
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_if() {
        let parsed_ast = parse("{% if superadmin %}Hey{% endif %}");
        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hey".to_string()));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_front(Node::Conditional {
            condition: Box::new(Node::Identifier {name: "superadmin".to_string(), filters: None}),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: None,
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_if_with_test() {
        let parsed_ast = parse("{% if number is defined %}Hey{% endif %}");
        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hey".to_string()));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_front(Node::Conditional {
            condition: Box::new(
                Node::Test {
                    expression: Box::new(Node::Identifier {
                        name: "number".to_string(), filters: None,
                    }),
                    name: "defined".to_string(),
                    params: LinkedList::new()
                }
            ),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: None,
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_if_with_test_params() {
        let parsed_ast = parse(r#"{% if pi is equalto 3.13 %}Hey{% endif %}"#);
        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hey".to_string()));

        let mut params = LinkedList::new();
        params.push_front(Node::Float(3.13));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_front(Node::Conditional {
            condition: Box::new(
                Node::Test {
                    expression: Box::new(Node::Identifier {
                        name: "pi".to_string(), filters: None,
                    }),
                    name: "equalto".to_string(),
                    params: params,
                }
            ),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: None,
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_if_elif_with_test() {
        let parsed_ast = parse("{% if hi %}Hey{% elif admin is oneof(a, 2, true) %}Hey{% endif %}");

        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hey".to_string()));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_front(Node::Conditional {
            condition: Box::new(Node::Identifier {name: "hi".to_string(), filters: None}),
            body: Box::new(Node::List(body.clone()))
        });

        let mut params = LinkedList::new();
        params.push_back(Node::Identifier { name: "a".to_string(), filters: None });
        params.push_back(Node::Int(2));
        params.push_back(Node::Bool(true));
        condition_nodes.push_back(Node::Conditional {
            condition: Box::new(
                Node::Test {
                    expression: Box::new(Node::Identifier {
                        name: "admin".to_string(), filters: None,
                    }),
                    name: "oneof".to_string(),
                    params: params,
                }
            ),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: None,
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }


    #[test]
    fn test_ast_if_else() {
        let parsed_ast = parse("{% if superadmin %}Hey{% else %}Hey{% endif %}");
        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hey".to_string()));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_front(Node::Conditional {
            condition: Box::new(Node::Identifier {name: "superadmin".to_string(), filters: None}),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: Some(Box::new(Node::List(body.clone()))),
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_if_elif() {
        let parsed_ast = parse("{% if superadmin %}Hey{% elif admin %}Hey{% endif %}");
        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hey".to_string()));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_front(Node::Conditional {
            condition: Box::new(Node::Identifier {name: "superadmin".to_string(), filters: None}),
            body: Box::new(Node::List(body.clone()))
        });
        condition_nodes.push_back(Node::Conditional {
            condition: Box::new(Node::Identifier {name: "admin".to_string(), filters: None}),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: None,
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_if_elifs_else() {
        let parsed_ast = parse("{% if superadmin %}Hey{% elif admin %}Hey{% else %}Hey{% endif %}");
        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hey".to_string()));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_back(Node::Conditional {
            condition: Box::new(Node::Identifier {name: "admin".to_string(), filters: None}),
            body: Box::new(Node::List(body.clone()))
        });
        condition_nodes.push_front(Node::Conditional {
            condition: Box::new(Node::Identifier {name: "superadmin".to_string(), filters: None}),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: Some(Box::new(Node::List(body.clone()))),
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_not_condition_and() {
        let parsed_ast = parse("{% if admin and not superadmin %}Admin{% endif %}");
        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Admin".to_string()));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_back(Node::Conditional {
            condition: Box::new(Node::Logic {
                lhs: Box::new(Node::Identifier {name: "admin".to_string(), filters: None}),
                rhs: Box::new(Node::Not(Box::new(Node::Identifier {name: "superadmin".to_string(), filters: None}))),
                operator: "and".to_string(),
            }),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: None,
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_not_condition_or() {
        let parsed_ast = parse("{% if not active or number_users > 10 %}Login{% endif %}");
        let mut ast = LinkedList::new();
        let mut body = LinkedList::new();
        body.push_front(Node::Text("Login".to_string()));

        let mut condition_nodes = LinkedList::new();
        condition_nodes.push_back(Node::Conditional {
            condition: Box::new(Node::Logic {
                lhs: Box::new(Node::Not(Box::new(Node::Identifier {name: "active".to_string(), filters: None}))),
                rhs: Box::new(Node::Logic {
                    lhs: Box::new(Node::Identifier {name: "number_users".to_string(), filters: None}),
                    rhs: Box::new(Node::Int(10)),
                    operator: ">".to_string(),
                }),
                operator: "or".to_string(),
            }),
            body: Box::new(Node::List(body.clone()))
        });

        ast.push_front(Node::If {
            condition_nodes: condition_nodes,
            else_node: None,
        });
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_nullary_test() {
        let mut parser = Rdp::new(StringInput::new("is defined"));
        assert!(parser.test());
        assert!(parser.end());
    }

    #[test]
    fn test_unary_test() {
        let mut parser = Rdp::new(StringInput::new("is equalto other"));
        assert!(parser.test());
        assert!(parser.end());

        let mut parser = Rdp::new(StringInput::new("is equalto(other)"));
        assert!(parser.test());
        assert!(parser.end());
    }

    #[test]
    fn test_n_ary_test() {
        let mut parser = Rdp::new(StringInput::new("is oneof(a, b, c)"));
        assert!(parser.test());
        assert!(parser.end());
    }

    // TODO: remove that syntax
    #[test]
    fn test_n_ary_test_requires_parens() {
        let mut parser = Rdp::new(StringInput::new("is oneof a, b, c"));
        assert!(parser.test()); // parse until the ','
        assert!(!parser.end());
    }

    #[test]
    fn test_ast_raw() {
        let parsed_ast = parse("Hey {% raw %}Hey {{ name }}{% endraw %} Ho");
        let mut ast = LinkedList::new();
        // A bit ugly to have a separate node for the leading ws but oh well
        ast.push_front(Node::Text(" Ho".to_string()));
        ast.push_front(Node::Raw("Hey {{ name }}".to_string()));
        ast.push_front(Node::Text("Hey ".to_string()));
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_filter() {
        let parsed_ast = parse("{{ greeting | i18n(lang=user.lang, units=user.units) | truncate(limit=50, cut_word=true) }}");
        let mut filters = LinkedList::new();

        let mut args_truncate = HashMap::new();
        args_truncate.insert("limit".to_string(), Node::Int(50));
        args_truncate.insert("cut_word".to_string(), Node::Bool(true));
        let mut args_i18n = HashMap::new();
        args_i18n.insert("lang".to_string(), Node::Identifier {name: "user.lang".to_string(), filters: None});
        args_i18n.insert("units".to_string(), Node::Identifier {name: "user.units".to_string(), filters: None});

        filters.push_front(Node::Filter {
            name: "truncate".to_string(),
            params: args_truncate,
        });
        filters.push_front(Node::Filter {
            name: "i18n".to_string(),
            params: args_i18n,
        });

        let mut ast = LinkedList::new();
        ast.push_front(Node::VariableBlock(
            Box::new(Node::Identifier {
                name: "greeting".to_string(),
                filters: Some(filters),
            })
        ));
        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_macro_definition_no_arg() {
        let parsed_ast = parse("{% macro helloworld() %}Hello{% endmacro helloworld %}");
        let params = LinkedList::new();

        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hello".to_string()));

        let mut ast = LinkedList::new();
        ast.push_front(Node::Macro {
            name: "helloworld".to_string(),
            params: params,
            body: Box::new(Node::List(body.clone())),
        });

        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_macro_definition_one_arg() {
        let parsed_ast = parse("{% macro helloworld(greeting) %}Hello{% endmacro helloworld %}");
        let mut params = LinkedList::new();
        params.push_front("greeting".to_string());

        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hello".to_string()));

        let mut ast = LinkedList::new();
        ast.push_front(Node::Macro {
            name: "helloworld".to_string(),
            params: params,
            body: Box::new(Node::List(body.clone())),
        });

        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_macro_definition_multiple_args() {
        let parsed_ast = parse("{% macro helloworld(greeting, language) %}Hello{% endmacro helloworld %}");
        let mut params = LinkedList::new();
        params.push_front("language".to_string());
        params.push_front("greeting".to_string());

        let mut body = LinkedList::new();
        body.push_front(Node::Text("Hello".to_string()));

        let mut ast = LinkedList::new();
        ast.push_front(Node::Macro {
            name: "helloworld".to_string(),
            params: params,
            body: Box::new(Node::List(body.clone())),
        });

        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_macro_import() {
        let parsed_ast = parse("{% import \"macros.html\" as macros %}");
        let mut ast = LinkedList::new();
        ast.push_front(Node::ImportMacro {
            tpl_name: "macros.html".to_string(),
            name: "macros".to_string(),
        });

        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_macro_call_no_args() {
        let parsed_ast = parse("{{ macros::macro1() }}");
        let mut ast = LinkedList::new();
        let params = HashMap::new();
        ast.push_front(Node::MacroCall {
            namespace: "macros".to_string(),
            name: "macro1".to_string(),
            params: params
        });

        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_macro_call_one_arg() {
        let parsed_ast = parse("{{ macros::macro1(foo=bar) }}");
        let mut ast = LinkedList::new();
        let mut params = HashMap::new();
        params.insert("foo".to_string(), Node::Identifier {name: "bar".to_string(), filters: None});
        ast.push_front(Node::MacroCall {
            namespace: "macros".to_string(),
            name: "macro1".to_string(),
            params: params
        });

        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_macro_call_multiple_args() {
        let parsed_ast = parse("{{ macros::macro1(foo=bar, hey=1+2) }}");
        let mut ast = LinkedList::new();
        let mut params = HashMap::new();
        params.insert("foo".to_string(), Node::Identifier {name: "bar".to_string(), filters: None});
        params.insert("hey".to_string(), Node::Math {
            lhs: Box::new(Node::Int(1)),
            rhs: Box::new(Node::Int(2)),
            operator: "+".to_string(),
        });
        ast.push_front(Node::MacroCall {
            namespace: "macros".to_string(),
            name: "macro1".to_string(),
            params: params
        });

        let root = Node::List(ast);
        assert_eq!(parsed_ast.unwrap(), root);
    }

    #[test]
    fn test_ast_error_old_and() {
        let parsed_ast = parse("{{ true && 1 }}");
        assert!(parsed_ast.is_err());
        assert_eq!(
            parsed_ast.err().unwrap().description(),
            "Use `and` instead of `&&` at line 1, column 9"
        );
    }

    #[test]
    fn test_ast_error_old_or() {
        let parsed_ast = parse("{{ true || 1 }}");
        assert!(parsed_ast.is_err());
        assert_eq!(
            parsed_ast.err().unwrap().description(),
            "Use `or` instead of `||` at line 1, column 9"
        );
    }

    #[test]
    fn test_ast_error_mismatch_endblock_name() {
        let parsed_ast = parse("{% block hey %}{% endblock ho %}");
        assert!(parsed_ast.is_err());
        assert_eq!(
            parsed_ast.err().unwrap().description(),
            "Block `ho` is closing at line 1, col 33 but we were expecting `hey` to be closing"
        );
    }

    // Test that we can parse the template used in benching
    #[test]
    fn test_parse_bench() {
        let parsed_ast = parse("
            <html>
              <head>
                <title>{{ product.name }}</title>
              </head>
              <body>
                <h1>{{ product.name }} - {{ product.manufacturer }}</h1>
                <p>{{ product.summary }}</p>
                <p>£{{ product.price * 1.20 }} (VAT inc.)</p>
                <p>Look at reviews from your friends {{ username }}</p>
                <button>Buy!</button>
              </body>
            </html>
        ");
        assert!(parsed_ast.is_ok());
    }
}
