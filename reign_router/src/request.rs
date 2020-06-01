use crate::{middleware::session::SessionData, ParamError};
use hyper::{
    body::{to_bytes, Bytes},
    http::Extensions,
    Body, Error, HeaderMap, Method, Request as HyperRequest, Uri, Version,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap as Map, net::SocketAddr};
use url::form_urlencoded::parse;

#[derive(Debug)]
pub struct Request {
    method: Method,
    version: Version,
    uri: Uri,
    headers: HeaderMap,
    ip: SocketAddr,
    pub(crate) params: Map<String, String>,
    pub(crate) query: Map<String, String>,
    pub extensions: Extensions,
}

impl Request {
    pub(crate) fn new(ip: SocketAddr, req: HyperRequest<Body>) -> Self {
        let mut ret = Self {
            method: req.method().clone(),
            version: req.version(),
            uri: req.uri().clone(),
            headers: req.headers().clone(),
            ip,
            params: Map::new(),
            query: req
                .uri()
                .query()
                .map(|v| parse(v.as_bytes()).into_owned().collect())
                .unwrap_or_else(Map::new),
            extensions: Extensions::new(),
        };

        ret.extensions.insert::<Body>(req.into_body());
        ret
    }

    #[inline]
    pub fn ip(&self) -> &SocketAddr {
        &self.ip
    }

    /// Returns a reference to the associated Method.
    #[inline]
    pub fn method(&self) -> &Method {
        &self.method
    }

    #[inline]
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Returns a reference to the associated URI.
    #[inline]
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Returns a reference to the associated HeaderMap.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Retrieve the Request body.
    pub async fn body(&mut self) -> Result<Option<Bytes>, Error> {
        if let Some(body) = self.extensions.remove::<Body>() {
            Some(to_bytes(body).await).transpose()
        } else {
            Ok(None)
        }
    }

    #[inline]
    pub fn query(&self, name: &str) -> Option<&String> {
        self.query.get(name)
    }

    pub fn param(&self, name: &str) -> Result<String, ParamError> {
        Ok(self
            .params
            .get(name)
            .ok_or(ParamError::RequiredParamNotFound(name.into()))?
            .clone())
    }

    pub fn param_opt(&self, name: &str) -> Result<Option<String>, ParamError> {
        Ok(self.params.get(name).cloned())
    }

    pub fn param_glob(&self, name: &str) -> Result<Vec<String>, ParamError> {
        Ok(self
            .params
            .get(name)
            .ok_or(ParamError::RequiredGlobParamNotFound(name.into()))?
            .clone()
            .split("/")
            .into_iter()
            .map(|x| x.into())
            .collect())
    }

    pub fn param_opt_glob(&self, name: &str) -> Result<Option<Vec<String>>, ParamError> {
        Ok(self
            .params
            .get(name)
            .cloned()
            .map(|x| x.split("/").into_iter().map(|x| x.into()).collect()))
    }

    pub fn session<T>(&mut self) -> Option<&T>
    where
        T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
    {
        self.extensions
            .get::<SessionData<T>>()
            .and_then(|data| match data {
                SessionData::Clean(data) => Some(data),
                _ => None,
            })
    }

    pub fn store_session<T>(&mut self, data: T)
    where
        T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
    {
        self.extensions.insert(SessionData::Dirty(data));
    }

    pub fn drop_session<T>(&mut self)
    where
        T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
    {
        if self.extensions.get::<SessionData<T>>().is_some() {
            self.extensions.insert(SessionData::<T>::None);
        }
    }
}
