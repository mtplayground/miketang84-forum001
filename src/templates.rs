use askama::Template;

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub page_title: &'a str,
    pub heading: &'a str,
    pub intro: &'a str,
    pub is_authenticated: bool,
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
    pub is_authenticated: bool,
}

impl RegistrationTemplate {
    pub fn new(
        form: RegistrationFormValues,
        errors: RegistrationErrors,
        is_authenticated: bool,
    ) -> Self {
        Self {
            page_title: "Register",
            form,
            errors,
            is_authenticated,
        }
    }
}

#[derive(Default)]
pub struct LoginFormValues {
    pub username: String,
}

#[derive(Default)]
pub struct LoginErrors {
    pub username: Option<String>,
    pub password: Option<String>,
    pub general: Option<String>,
}

impl LoginErrors {
    pub fn is_empty(&self) -> bool {
        self.username.is_none() && self.password.is_none() && self.general.is_none()
    }
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub page_title: &'static str,
    pub form: LoginFormValues,
    pub errors: LoginErrors,
    pub is_authenticated: bool,
}

impl LoginTemplate {
    pub fn new(form: LoginFormValues, errors: LoginErrors, is_authenticated: bool) -> Self {
        Self {
            page_title: "Login",
            form,
            errors,
            is_authenticated,
        }
    }
}

#[derive(Template)]
#[template(path = "profile.html")]
pub struct ProfileTemplate {
    pub page_title: String,
    pub username: String,
    pub joined_on: String,
    pub bio: String,
    pub post_count: i64,
    pub is_authenticated: bool,
}

impl ProfileTemplate {
    pub fn from_user(user: crate::models::User, is_authenticated: bool, post_count: i64) -> Self {
        let page_title = format!("{}'s profile", user.username);
        let joined_on = user.created_at.format("%B %-d, %Y").to_string();
        let bio = if user.bio.trim().is_empty() {
            "This user has not written a bio yet.".to_owned()
        } else {
            user.bio
        };

        Self {
            page_title,
            username: user.username,
            joined_on,
            bio,
            post_count,
            is_authenticated,
        }
    }
}
