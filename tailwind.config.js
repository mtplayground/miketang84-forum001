/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./templates/**/*.html",
    "./src/**/*.rs"
  ],
  theme: {
    extend: {
      colors: {
        parchment: {
          50: "#fbf7f0",
          100: "#f3ebdf"
        },
        ink: {
          900: "#241a12"
        },
        ember: {
          500: "#8c5d2d"
        }
      },
      boxShadow: {
        panel: "0 1rem 3rem rgba(71, 43, 20, 0.08)"
      }
    }
  },
  plugins: []
};
