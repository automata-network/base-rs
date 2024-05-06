
#[macro_export]
macro_rules! stack_error {
    (
        name: $name:ident,
        stack_name: $stack_ty_name:ident,
        error: {
            $($err_name:ident $(($($err_tuple:ty),*))? $( { $($err_field:ident : $err_field_type:ty),* } )? ),* $(,)*
        },
        stack: {
            $($stack_name:ident( $($stack_field:ident : $stack_field_type:ty),* ),)*
        }
    ) => {
        #[derive(Debug, PartialEq)]
        pub enum $name {
            $(
                $err_name $(
                    ($($err_tuple),*)
                )? $(
                    { $($err_field : $err_field_type),* }
                )?,
            )*
            Stack { origin: Box<$name>, stack: Vec<$stack_ty_name> },
        }

        #[derive(Debug, PartialEq)]
        pub enum $stack_ty_name {
            $(
                $stack_name {
                    $($stack_field : $stack_field_type),*
                },
            )*
        }

        impl $name {
            $(
            #[allow(non_snake_case)]
            pub fn $stack_name<'a>($($stack_field : &'a $stack_field_type),*) -> Box<dyn FnOnce(Self) -> Self + 'a> {
                Box::new(move |origin| {
                    let stack_info = $stack_ty_name::$stack_name {
                        $($stack_field : $stack_field.clone() ),*
                    };
                    match origin {
                        Self::Stack{origin, mut stack} => {
                            stack.push(stack_info);
                            Self::Stack{ origin, stack }
                        }
                        origin => Self::Stack {
                            origin: Box::new(origin), 
                            stack: vec![stack_info],
                        }
                    }
                })
            }
            )*
        }
    }
}
