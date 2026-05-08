use askama::Template;

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub page_title: &'a str,
    pub heading: &'a str,
    pub intro: &'a str,
}

#[derive(Default)]
pub struct RegistrationFormValues {
    pub username: String,
}

#[derive(Default)]
pub struct RegistrationErrors {
    pub username: Option<String>,
    pub password: Option<String>,
    pub confirm: Option<String>,
}

impl RegistrationErrors {
    pub fn is_empty(&self) -> bool {
        self.username.is_none() && self.password.is_none() && self.confirm.is_none()
    }
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegistrationTemplate {
    pub page_title: &'static str,
    pub form: RegistrationFormValues,
    pub errors: RegistrationErrors,
}

impl RegistrationTemplate {
    pub fn new(form: RegistrationFormValues, errors: RegistrationErrors) -> Self {
        Self {
            page_title: "Register",
            form,
            errors,
        }
    }
}
