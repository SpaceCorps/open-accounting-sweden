// Local development without a GitHub Packages token: point @spacecorps/components at a checkout
// of SpaceCorps/storybook (built with `vp pack`):
//   OAS_STORYBOOK_PATH=~/git/spacecorps/storybook pnpm install
module.exports = {
  hooks: {
    readPackage(pkg) {
      const local = process.env.OAS_STORYBOOK_PATH;
      if (local && pkg.dependencies && pkg.dependencies["@spacecorps/components"]) {
        pkg.dependencies["@spacecorps/components"] = `file:${local}`;
      }
      return pkg;
    },
  },
};
