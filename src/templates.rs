use crate::models::Category;
use crate::models::Thread;
use crate::pagination::Pagination;
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

    pub fn from_category_with_counts(
        category_with_counts: crate::categories::CategoryWithCountsRow,
    ) -> Self {
        Self::from_category(
            Category {
                id: category_with_counts.id,
                slug: category_with_counts.slug,
                name: category_with_counts.name,
                description: category_with_counts.description,
                position: category_with_counts.position,
                created_at: category_with_counts.created_at,
            },
            category_with_counts.thread_count,
            category_with_counts.post_count,
        )
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
    pub slug: String,
    pub name: String,
    pub description: String,
    pub thread_count: i64,
    pub post_count: i64,
    pub threads: Vec<CategoryThreadListItem>,
    pub pagination: Pagination,
    pub is_authenticated: bool,
}

impl CategoryDetailTemplate {
    pub fn from_category_with_counts(
        category_with_counts: crate::categories::CategoryWithCountsRow,
        is_authenticated: bool,
        threads: Vec<CategoryThreadListItem>,
        pagination: Pagination,
    ) -> Self {
        Self {
            page_title: category_with_counts.name.clone(),
            slug: category_with_counts.slug,
            name: category_with_counts.name,
            description: if category_with_counts.description.trim().is_empty() {
                "No description yet.".to_owned()
            } else {
                category_with_counts.description
            },
            thread_count: category_with_counts.thread_count,
            post_count: category_with_counts.post_count,
            threads,
            pagination,
            is_authenticated,
        }
    }
}

pub struct CategoryThreadListItem {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub is_pinned: bool,
    pub last_activity_on: String,
}

impl CategoryThreadListItem {
    pub fn from_thread(thread: Thread) -> Self {
        Self {
            id: thread.id,
            title: thread.title,
            slug: thread.slug,
            is_pinned: thread.is_pinned,
            last_activity_on: thread.last_activity_at.format("%B %-d, %Y").to_string(),
        }
    }
}

pub struct AdminCategoryListItem {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub position: i32,
    pub thread_count: i64,
    pub post_count: i64,
}

impl AdminCategoryListItem {
    pub fn from_row(category_with_counts: crate::categories::CategoryWithCountsRow) -> Self {
        Self {
            id: category_with_counts.id,
            slug: category_with_counts.slug,
            name: category_with_counts.name,
            description: category_with_counts.description,
            position: category_with_counts.position,
            thread_count: category_with_counts.thread_count,
            post_count: category_with_counts.post_count,
        }
    }
}

#[derive(Default, Clone)]
pub struct AdminCategoryFormValues {
    pub name: String,
    pub description: String,
    pub position: String,
}

#[derive(Default, Clone)]
pub struct AdminCategoryFormErrors {
    pub name: Option<String>,
    pub position: Option<String>,
    pub general: Option<String>,
}

impl AdminCategoryFormErrors {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.position.is_none() && self.general.is_none()
    }
}

#[derive(Template)]
#[template(path = "admin_categories.html")]
pub struct AdminCategoryListTemplate {
    pub page_title: &'static str,
    pub categories: Vec<AdminCategoryListItem>,
    pub form: AdminCategoryFormValues,
    pub errors: AdminCategoryFormErrors,
    pub is_authenticated: bool,
}

impl AdminCategoryListTemplate {
    pub fn new(
        categories: Vec<AdminCategoryListItem>,
        form: AdminCategoryFormValues,
        errors: AdminCategoryFormErrors,
    ) -> Self {
        Self {
            page_title: "Admin Categories",
            categories,
            form,
            errors,
            is_authenticated: true,
        }
    }
}

#[derive(Template)]
#[template(path = "admin_category_edit.html")]
pub struct AdminCategoryEditTemplate {
    pub page_title: &'static str,
    pub category_id: i64,
    pub form: AdminCategoryFormValues,
    pub errors: AdminCategoryFormErrors,
    pub is_authenticated: bool,
}

impl AdminCategoryEditTemplate {
    pub fn new(
        category_id: i64,
        form: AdminCategoryFormValues,
        errors: AdminCategoryFormErrors,
    ) -> Self {
        Self {
            page_title: "Edit Category",
            category_id,
            form,
            errors,
            is_authenticated: true,
        }
    }
}

#[derive(Default)]
pub struct CreateThreadFormValues {
    pub title: String,
    pub body: String,
}

#[derive(Default)]
pub struct CreateThreadErrors {
    pub title: Option<String>,
    pub body: Option<String>,
}

impl CreateThreadErrors {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.body.is_none()
    }
}

#[derive(Template)]
#[template(path = "create_thread.html")]
pub struct CreateThreadTemplate {
    pub page_title: String,
    pub category_slug: String,
    pub category_name: String,
    pub form: CreateThreadFormValues,
    pub errors: CreateThreadErrors,
    pub is_authenticated: bool,
}

impl CreateThreadTemplate {
    pub fn new(
        category_slug: String,
        category_name: String,
        form: CreateThreadFormValues,
        errors: CreateThreadErrors,
    ) -> Self {
        let page_title = format!("New Thread in {category_name}");

        Self {
            page_title,
            category_slug,
            category_name,
            form,
            errors,
            is_authenticated: true,
        }
    }
}

#[derive(Template)]
#[template(path = "thread_detail.html")]
pub struct ThreadDetailTemplate {
    pub thread_id: i64,
    pub page_title: String,
    pub canonical_path: String,
    pub category_slug: String,
    pub category_name: String,
    pub title: String,
    pub is_pinned: bool,
    pub opening_post: ThreadPostItem,
    pub replies: Vec<ThreadPostItem>,
    pub reply_count: i64,
    pub pagination: Pagination,
    pub is_authenticated: bool,
    pub is_admin: bool,
    pub is_locked: bool,
    pub reply_form: ReplyFormValues,
    pub reply_errors: ReplyErrors,
}

impl ThreadDetailTemplate {
    pub fn new(
        thread_id: i64,
        thread_slug: String,
        category_slug: String,
        category_name: String,
        title: String,
        is_pinned: bool,
        opening_post: ThreadPostItem,
        replies: Vec<ThreadPostItem>,
        reply_count: i64,
        pagination: Pagination,
        is_authenticated: bool,
        is_admin: bool,
        is_locked: bool,
        reply_form: ReplyFormValues,
        reply_errors: ReplyErrors,
    ) -> Self {
        Self {
            thread_id,
            page_title: title.clone(),
            canonical_path: format!("/t/{thread_id}-{thread_slug}"),
            category_slug,
            category_name,
            title,
            is_pinned,
            opening_post,
            replies,
            reply_count,
            pagination,
            is_authenticated,
            is_admin,
            is_locked,
            reply_form,
            reply_errors,
        }
    }
}

#[derive(Default)]
pub struct EditPostFormValues {
    pub body: String,
    pub return_page: i64,
}

#[derive(Default)]
pub struct EditPostErrors {
    pub body: Option<String>,
    pub general: Option<String>,
}

impl EditPostErrors {
    pub fn is_empty(&self) -> bool {
        self.body.is_none() && self.general.is_none()
    }
}

#[derive(Template)]
#[template(path = "edit_post.html")]
pub struct EditPostTemplate {
    pub page_title: &'static str,
    pub canonical_path: String,
    pub form_action: String,
    pub thread_title: String,
    pub form: EditPostFormValues,
    pub errors: EditPostErrors,
    pub is_authenticated: bool,
}

impl EditPostTemplate {
    pub fn new(
        canonical_path: String,
        form_action: String,
        thread_title: String,
        form: EditPostFormValues,
        errors: EditPostErrors,
    ) -> Self {
        Self {
            page_title: "Edit Post",
            canonical_path,
            form_action,
            thread_title,
            form,
            errors,
            is_authenticated: true,
        }
    }
}

#[derive(Default)]
pub struct ReplyFormValues {
    pub body: String,
}

#[derive(Default)]
pub struct ReplyErrors {
    pub body: Option<String>,
    pub general: Option<String>,
}

impl ReplyErrors {
    pub fn is_empty(&self) -> bool {
        self.body.is_none() && self.general.is_none()
    }
}

pub struct ThreadPostItem {
    pub author_username: String,
    pub created_at: String,
    pub edited_at: Option<String>,
    pub body_html: String,
    pub is_deleted: bool,
    pub show_deleted_content: bool,
    pub deleted_notice: &'static str,
    pub can_edit: bool,
    pub can_delete: bool,
    pub can_admin_delete: bool,
    pub edit_path: String,
    pub delete_path: String,
    pub admin_delete_path: String,
    pub return_page: i64,
}

impl ThreadPostItem {
    pub fn new(
        author_username: String,
        created_at: chrono::DateTime<chrono::Utc>,
        edited_at: Option<chrono::DateTime<chrono::Utc>>,
        body_html: String,
        is_deleted: bool,
        show_deleted_content: bool,
        can_edit: bool,
        can_delete: bool,
        can_admin_delete: bool,
        edit_path: String,
        delete_path: String,
        admin_delete_path: String,
        return_page: i64,
    ) -> Self {
        Self {
            author_username,
            created_at: created_at.format("%B %-d, %Y at %-I:%M %p UTC").to_string(),
            edited_at: (!is_deleted).then_some(edited_at).flatten().map(|timestamp| {
                format!("Edited {}", timestamp.format("%B %-d, %Y at %-I:%M %p UTC"))
            }),
            body_html,
            is_deleted,
            show_deleted_content,
            deleted_notice: "This post has been deleted by its author.",
            can_edit,
            can_delete,
            can_admin_delete,
            edit_path,
            delete_path,
            admin_delete_path,
            return_page,
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
