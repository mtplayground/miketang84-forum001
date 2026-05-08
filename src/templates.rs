use crate::models::Category;
use askama::Template;

pub struct CategoryIndexItem {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub thread_count: i64,
    pub post_count: i64,
}

impl CategoryIndexItem {
    pub fn from_category(category: Category, thread_count: i64, post_count: i64) -> Self {
        let description = if category.description.trim().is_empty() {
            "No description yet.".to_owned()
        } else {
            category.description
        };

        Self {
            slug: category.slug,
            name: category.name,
            description,
            thread_count,
            post_count,
        }
    }
}

#[derive(Template)]
#[template(path = "categories_index.html")]
pub struct CategoryIndexTemplate {
    pub page_title: &'static str,
    pub categories: Vec<CategoryIndexItem>,
    pub is_authenticated: bool,
}

#[derive(Template)]
#[template(path = "category_detail.html")]
pub struct CategoryDetailTemplate {
    pub page_title: String,
    pub name: String,
    pub description: String,
    pub thread_count: i64,
    pub post_count: i64,
    pub is_authenticated: bool,
}

impl CategoryDetailTemplate {
    pub fn from_category(
        category: Category,
        is_authenticated: bool,
        thread_count: i64,
        post_count: i64,
    ) -> Self {
        let page_title = category.name.clone();
        let description = if category.description.trim().is_empty() {
            "No description yet.".to_owned()
        } else {
            category.description
        };

        Self {
            page_title,
            name: category.name,
            description,
            thread_count,
            post_count,
            is_authenticated,
        }
    }
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

#[derive(Default)]
pub struct ProfileSettingsFormValues {
    pub bio: String,
}

#[derive(Default)]
pub struct ProfileSettingsErrors {
    pub bio: Option<String>,
}

impl ProfileSettingsErrors {
    pub fn is_empty(&self) -> bool {
        self.bio.is_none()
    }
}

#[derive(Template)]
#[template(path = "settings_profile.html")]
pub struct ProfileSettingsTemplate {
    pub page_title: &'static str,
    pub username: String,
    pub form: ProfileSettingsFormValues,
    pub errors: ProfileSettingsErrors,
    pub is_authenticated: bool,
    pub saved: bool,
}

impl ProfileSettingsTemplate {
    pub fn new(
        username: String,
        form: ProfileSettingsFormValues,
        errors: ProfileSettingsErrors,
        saved: bool,
    ) -> Self {
        Self {
            page_title: "Edit Profile",
            username,
            form,
            errors,
            is_authenticated: true,
            saved,
        }
    }
}

#[derive(Default)]
pub struct PasswordSettingsErrors {
    pub current_password: Option<String>,
    pub new_password: Option<String>,
}

impl PasswordSettingsErrors {
    pub fn is_empty(&self) -> bool {
        self.current_password.is_none() && self.new_password.is_none()
    }
}

#[derive(Template)]
#[template(path = "settings_password.html")]
pub struct PasswordSettingsTemplate {
    pub page_title: &'static str,
    pub username: String,
    pub errors: PasswordSettingsErrors,
    pub is_authenticated: bool,
    pub saved: bool,
}

impl PasswordSettingsTemplate {
    pub fn new(username: String, errors: PasswordSettingsErrors, saved: bool) -> Self {
        Self {
            page_title: "Change Password",
            username,
            errors,
            is_authenticated: true,
            saved,
        }
    }
}
