#[macro_export]
macro_rules! routes {
    ( $(
        $route:literal => $id:ident
        {
            $(
                $( #[$field_meta:meta] )*
                    $field_vis:vis $field_name:ident : $field_ty:ty
            ),*
                $(,)?
        }
    )* ) => {
        $crate::paste::paste! {
            #[derive(Debug, Clone, PartialEq)]
            pub enum NamedRoute {
                $(
                    $id([<$id Path>]),
                )*
            }

            impl NamedRoute {
                pub fn to_href(&self) -> String {
                    // NOTE: this uses the Display implementation derived by the TypedPath macro
                    match self {
                        $(Self::$id(path) => path.to_string(),)*
                    }
                }
            }

            #[axum::async_trait]
            impl<S> axum::extract::FromRequestParts<S> for NamedRoute
            where
                S: Send + Sync,
            {
                type Rejection = ();

                async fn from_request_parts(req: &mut http::request::Parts, s: &S) -> Result<Self, Self::Rejection> {
                    let path = if let Some(path) = req.extensions.get::<axum::extract::MatchedPath>() {
                        path.as_str()
                    } else {
                        return Err(());
                    };

                    match path {
                        $(
                            [<$id Path>]::PATH => {
                                let p = [<$id Path>]::from_request_parts(req, s).await.expect("we have matched the path, so the Path extractor should");
                                Ok(Self::$id(p))
                            },
                        )*
                            _ => Err(())
                    }
                }
            }


            $(
                #[derive(muxa::reexports::TypedPath, Debug, muxa::reexports::Deserialize, Clone, PartialEq)]
                #[typed_path($route)]
                pub struct [<$id Path>] {
                    $(
                        $( #[$field_meta] )*
                            $field_vis $field_name : $field_ty
                    ),*
                }

                #[allow(dead_code)]
                pub fn [<route_ $id:snake>](
                    $(
                        $field_name : $field_ty
                    ),*
                ) -> NamedRoute {
                    NamedRoute::$id([<$id Path>] {
                        $(
                            $field_name
                        ),*
                    })
                }
            )*
        }

        impl NamedRoute {
            pub fn redirect(&self) -> axum::response::Redirect {
                axum::response::Redirect::to(&self.to_href())
            }

            /// returns true if the routes are the same, ignoring the params
            /// if you want equality of params too, use PartialEq::eq instead
            pub fn matches(&self, other: &NamedRoute) -> bool {
                use std::mem::discriminant;
                discriminant(self) == discriminant(other)
            }
        }

        impl std::fmt::Display for NamedRoute {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_href())
            }
        }

        impl maud::Render for NamedRoute {
            fn render_to(&self, w: &mut String) {
                w.push_str(&self.to_href());
            }
        }
    };
}
