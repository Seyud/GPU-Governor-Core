/// 简化的 getter/setter 宏，用于生成带有调试日志的属性访问方法
#[macro_export]
macro_rules! getter_setter {
    ($field:ident: $type:ty, $debug_msg:literal) => {
        paste::paste! {
            pub fn [<get_ $field>](&self) -> $type {
                self.$field
            }

            pub fn [<set_ $field>](&mut self, $field: $type) {
                self.$field = $field;
                log::debug!($debug_msg, $field);
            }
        }
    };
}

/// 简单的 getter 宏，用于生成只读属性访问方法
#[macro_export]
macro_rules! simple_getter {
    ($field:ident: $type:ty) => {
        paste::paste! {
            pub fn [<get_ $field>](&self) -> $type {
                self.$field
            }
        }
    };
}

/// 简单的 getter/setter 宏，用于生成属性访问方法（无调试日志）
#[macro_export]
macro_rules! simple_getter_setter {
    ($field:ident: $type:ty) => {
        paste::paste! {
            pub fn [<get_ $field>](&self) -> $type {
                self.$field.clone()
            }

            pub fn [<set_ $field>](&mut self, $field: $type) {
                self.$field = $field;
            }
        }
    };
}

/// 简单的 setter 宏，用于生成只写属性设置方法
#[macro_export]
macro_rules! simple_setter {
    ($field:ident: $type:ty) => {
        paste::paste! {
            pub fn [<set_ $field>](&mut self, $field: $type) {
                self.$field = $field;
            }
        }
    };
}
