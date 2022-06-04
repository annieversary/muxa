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
            impl<B> axum::extract::FromRequest<B> for NamedRoute
            where
              B: Send,
            {
              type Rejection = ();

              async fn from_request(req: &mut axum::extract::RequestParts<B>) -> Result<Self, Self::Rejection> {
                let path = if let Some(path) = req.extensions().get::<axum::extract::MatchedPath>() {
                  path.as_str()
                } else {
                  return Err(());
                };

                match path {
                  $(
                    [<$id Path>]::PATH => {
                      let p = [<$id Path>]::from_request(req).await.expect("we have matched the path, so the Path extractor should");
                      Ok(Self::$id(p))
                    },
                  )*
                    _ => Err(())
                }
              }
            }


            $(
              #[derive(TypedPath, Debug, Deserialize, Clone, PartialEq)]
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
    };
}
