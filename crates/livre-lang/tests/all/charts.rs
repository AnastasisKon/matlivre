//! Charts (heatmap, boxchart, swarmchart, bubble charts, binscatter, piechart, donutchart,
//! bubblecloud, parallelplot, stackedplot, scatterhistogram, wordcloud, geobubble), yyaxis and
//! datetick against MATLAB R2023b. Layouts that MATLAB computes from the screen (word and bubble
//! placement, automatic bin edges) are not compared.

use livre_lang::{Interpreter, OutputKind};

fn shown(src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    let _ = it.run_source(src);
    it.take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Value { text, .. } => Some(text),
            OutputKind::Text(t) => Some(t),
            OutputKind::Warning(w) => Some(format!("Warning: {w}")),
            OutputKind::Error(e) => Some(format!("Error: {e}")),
            _ => None,
        })
        .collect()
}

#[test]
fn heatmaps_match_matlab() {
    assert_eq!(
        shown(concat!(
            "h = heatmap([1 2 3; 4 5 6]); c = class(h)\nxl = h.XDisplayLabels'\ncd = h.ColorData\nlim = h.ColorLimits\nfmt = h.CellLabelFormat\nh\n",
            "T = table({'a';'b';'a';'b';'a'}, {'x';'x';'y';'y';'y'}, [1;2;3;4;5], 'VariableNames', {'R','C','V'});\n",
            "h2 = heatmap(T, 'C', 'R'); c2 = h2.ColorData\nm2 = h2.ColorMethod\nt2 = h2.Title\n",
            "h3 = heatmap(T, 'C', 'R', 'ColorVariable', 'V')\nc3 = h3.ColorData\nh3.ColorMethod = 'max'; t3 = h3.Title\n",
            "h4 = heatmap({'p','q'}, {'r','s','t'}, [8 1; 3 5; 4 9]); x4 = h4.XData'\nh4.Title = 'My map'; t4 = h4.Title\n",
            "h4.XDisplayLabels = {'a','b'}; d4 = h4.XDisplayLabels'\n",
        )),
        [
            "c = 'matlab.graphics.chart.HeatmapChart'",
            "xl = 1×3 cell\n\n    {'1'}    {'2'}    {'3'}",
            "cd = 2×3\n\n     1     2     3\n     4     5     6",
            "lim = 1×2\n\n     1     6",
            "fmt = '%0.4g'",
            "h = \n  HeatmapChart with properties:\n\n        XData: {3×1 cell}\n        YData: {2×1 cell}\n    ColorData: [2×3 double]\n\n  Use GET to show all properties",
            "c2 = 2×2\n\n     1     2\n     1     1",
            "m2 = 'count'",
            "t2 = 'Count of R vs. C'",
            "h3 = \n  HeatmapChart (Mean of V) with properties:\n\n      SourceTable: [5×3 table]\n        XVariable: 'C'\n        YVariable: 'R'\n    ColorVariable: 'V'\n      ColorMethod: 'mean'\n\n  Use GET to show all properties",
            "c3 = 2×2\n\n     1     4\n     2     4",
            "t3 = 'Maximum of V'",
            "x4 = 1×2 cell\n\n    {'p'}    {'q'}",
            "t4 = 'My map'",
            "d4 = 1×2 cell\n\n    {'a'}    {'b'}",
        ]
    );
}

#[test]
fn box_swarm_and_bubble_charts_match_matlab() {
    assert_eq!(
        shown(concat!(
            "b = boxchart([1 2 3 4 5 10]'); c = class(b)\nw = b.BoxWidth\no = b.Orientation\nb\n",
            "b3 = boxchart([1 1 2 2], [1 2 3 4]); x3 = b3.XData\nb4 = boxchart([1 2 3 4], 'GroupByColor', [1 1 2 2]); s4 = size(b4)\n",
            "s = swarmchart([1 1 1 2 2], [1 2 3 4 5]); cs = class(s)\nj = s.XJitter\njw = s.XJitterWidth\nyj = s.YJitter\n",
            "bb = bubblechart([1 2 3], [4 5 6], [10 20 30]); cb = class(bb)\nfa = bb.MarkerFaceAlpha\nbs = bubblesize\nbl = bubblelim\nbb\n",
            "bubblesize([5 20]); bs2 = bubblesize\nbubblelim([0 40]); bl2 = bubblelim\n",
            "lg = bubblelegend('Sizes'); cl = class(lg)\nlt = lg.Title.String\nnb = lg.NumBubbles\nlg\n",
            "bb3 = bubblechart3([1 2], [3 4], [5 6], [1 2]); z3 = bb3.ZData\n",
        )),
        [
            "c = 'matlab.graphics.chart.primitive.BoxChart'",
            "w = 0.5000",
            "o = 'vertical'",
            "b = \n  BoxChart with properties:\n\n    XData: [6×1 categorical]\n    YData: [6×1 double]\n\n  Use GET to show all properties",
            "x3 = 1×4\n\n     1     1     2     2",
            "s4 = 1×2\n\n     2     1",
            "cs = 'matlab.graphics.chart.primitive.Scatter'",
            "j = 'density'",
            "jw = 0.9000",
            "yj = 'none'",
            "cb = 'matlab.graphics.chart.primitive.BubbleChart'",
            "fa = 0.6000",
            "bs = 1×2\n\n     3    50",
            "bl = 1×2\n\n    10    30",
            "bb = \n  BubbleChart with properties:\n\n    MarkerEdgeColor: 'flat'\n    MarkerFaceColor: 'flat'\n          LineWidth: 0.5000\n              XData: [1 2 3]\n              YData: [4 5 6]\n           SizeData: [10 20 30]\n              CData: [0 0.4470 0.7410]\n\n  Use GET to show all properties",
            "bs2 = 1×2\n\n     5    20",
            "bl2 = 1×2\n\n     0    40",
            "cl = 'matlab.graphics.illustration.BubbleLegend'",
            "lt = 'Sizes'",
            "nb = 3",
            "lg = \n  BubbleLegend (Sizes) with properties:\n\n    Location: 'northeast'\n       Style: 'vertical'\n    FontSize: 9\n    Position: [0.6810 0.4677 0.2054 0.4331]\n       Units: 'normalized'\n\n  Use GET to show all properties",
            "z3 = 1×2\n\n     5     6",
        ]
    );
}

#[test]
fn pie_and_figure_charts_match_matlab() {
    assert_eq!(
        shown(concat!(
            "p = piechart([1 2 3]); c = class(p)\nl = p.Labels\npr = p.Proportions\np\n",
            "p2 = piechart([1 2 3], {'a','b','c'}); l2 = p2.Labels\n",
            "p3 = piechart(categorical({'x','y','x','x'})); n3 = p3.Names\ncc = p3.CategoryCounts\nl3 = p3.Labels\n",
            "p4 = piechart([1 30]); l4 = p4.Labels\np5 = piechart([0.1 0.2]); l5 = p5.Labels\np6 = piechart([2 0 3]); l6 = p6.Labels\n",
            "d = donutchart([1 2 3]); r = d.InnerRadius\n",
            "bc = bubblecloud([10 20 30], {'a','b','c'})\n",
            "pp = parallelplot([1 2 3; 4 5 6; 7 8 9])\nct = pp.CoordinateTickLabels'\nnm = pp.DataNormalization\n",
            "sp = stackedplot([1 2 3; 4 5 6]'); dl = sp.DisplayLabels'\n",
            "sp2 = stackedplot(table([1;2;3], [4;5;6], 'VariableNames', {'A','B'})); dv = sp2.DisplayVariables\nxl = sp2.XLabel\n",
            "sh = scatterhistogram([1 2 3 4], [4 3 2 1])\nnb = sh.NumBins'\nst = sh.HistogramDisplayStyle\n",
            "wc = wordcloud({'apple','pear','fig'}, [3 2 1])\nwc2 = wordcloud(categorical({'b','a','b','c','c','c'})); w2 = wc2.WordData\ns2 = wc2.SizeData\n",
            "gb = geobubble([40 41], [-74 -73], [10 20])\n",
        )),
        [
            "c = 'matlab.graphics.chart.PieChart'",
            "l = 1×3 string array\n\n    \"16.7%\"    \"33.3%\"    \"50%\"",
            "pr = 1×3\n\n    0.1667    0.3333    0.5000",
            "p = \n  PieChart with properties:\n\n    ColorOrder: [7×3 double]\n     FaceAlpha: 0.6000\n     EdgeColor: [0.1500 0.1500 0.1500]\n        Labels: [\"16.7%\"    \"33.3%\"    \"50%\"]\n          Data: [1 2 3]\n         Names: [\"data1\"    \"data2\"    \"data3\"]\n\n  Use GET to show all properties",
            "l2 = 1×3 string array\n\n    \"a (16.7%)\"    \"b (33.3%)\"    \"c (50%)\"",
            "n3 = 1×2 string array\n\n    \"x\"    \"y\"",
            "cc = 1×2\n\n     3     1",
            "l3 = 1×2 string array\n\n    \"x (75%)\"    \"y (25%)\"",
            "l4 = 1×2 string array\n\n    \"3.23%\"    \"96.8%\"",
            "l5 = 1×2 string array\n\n    \"10%\"    \"20%\"",
            "l6 = 1×3 string array\n\n    \"40%\"    \"0%\"    \"60%\"",
            "r = 0.6000",
            "bc = \n  BubbleCloud with properties:\n\n     SizeData: [10 20 30]\n    LabelData: {'a'  'b'  'c'}\n    GroupData: [1×0 double]\n\n  Use GET to show all properties",
            "pp = \n  ParallelCoordinatesPlot with properties:\n\n              Data: [3×3 double]\n    CoordinateData: [1 2 3]\n         GroupData: []\n\n  Use GET to show all properties",
            "ct = '123'",
            "nm = 'range'",
            "dl = 1×2 cell\n\n    {'Column 1'}    {'Column 2'}",
            "dv = 1×2 cell\n\n    {'A'}    {'B'}",
            "xl = 'Row'",
            "sh = \n  ScatterHistogramChart with properties:\n\n        XData: [4×1 double]\n        YData: [4×1 double]\n    GroupData: []\n\n  Use GET to show all properties",
            "nb = 1×2\n\n     4     4",
            "st = 'stairs'",
            "wc = \n  WordCloudChart with properties:\n\n           WordData: [\"apple\"    \"pear\"    \"fig\"]\n           SizeData: [3 2 1]\n    MaxDisplayWords: 100\n\n  Use GET to show all properties",
            "w2 = 1×3 string array\n\n    \"c\"    \"b\"    \"a\"",
            "s2 = 1×3\n\n     3     2     1",
            "gb = \n  GeographicBubbleChart with properties:\n\n          Basemap: 'streets-light'\n        MapLayout: 'normal'\n     LatitudeData: [2×1 double]\n    LongitudeData: [2×1 double]\n         SizeData: [2×1 double]\n        ColorData: []\n\n  Use GET to show all properties",
        ]
    );
}

#[test]
fn binscatter_yyaxis_and_datetick_match_matlab() {
    assert_eq!(
        shown(concat!(
            "bs = binscatter(1:10, [2 4 6 8 10 9 7 5 3 1], 3); c = class(bs)\nn = bs.NumBins\nv = bs.Values\nbs2 = binscatter([0 1 2], [5 5 6], [2 3]); v2 = bs2.Values\n",
            "figure; yyaxis left; plot(1:3); plot(2:4); yyaxis right; h = plot([30 20 10]); ax = gca;\ncol = h.Color\nyl = ax.YLim\nloc = ax.YAxisLocation\n",
            "yyaxis left; yl2 = ax.YLim\nk = numel(ax.Children)\nsz = size(ax.YAxis)\nrc = ax.YAxis(2).Color\n",
            "yyaxis right; ylabel('R'); ylim([0 50]); yyaxis left; rl = ax.YAxis(2).Label.String\nrlim = ax.YAxis(2).Limits\nll = ax.YLim\n",
            "figure; plot(1:3, 'r'); yyaxis right; lc = ax.YAxisLocation\nax2 = gca; lc2 = ax2.YAxisLocation\nc1 = ax2.YAxis(1).Color\n",
            "figure; t = datenum(2020,1,1):30:datenum(2020,6,1); plot(t, 1:numel(t)); datetick('x', 'mmm'); lab = get(gca, 'XTickLabel')\ntk = get(gca, 'XTick')\n",
            "figure; t = datenum(2010,1,1):100:datenum(2020,1,1); plot(t, t); datetick; lab2 = get(gca, 'XTickLabel')\n",
            "figure; t = datenum(2020,1,1,0,0,0):1/24:datenum(2020,1,1,12,0,0); plot(t, t); datetick; lab3 = get(gca, 'XTickLabel')\n",
            "figure; t = datenum(2020,1,1):1:datenum(2020,1,10); plot(t, t); datetick('x', 'dd', 'keeplimits'); lab4 = get(gca, 'XTickLabel')\n",
            "datetick('y', 2); lab5 = get(gca, 'YTickLabel')\n",
        )),
        [
            "c = 'matlab.graphics.chart.primitive.Binscatter'",
            "n = 1×2\n\n     3     3",
            "v = 3×3\n\n     2     1     1\n     0     1     2\n     2     1     0",
            "v2 = 2×3\n\n     2     0     0\n     0     0     1",
            "col = 1×3\n\n    0.8500    0.3250    0.0980",
            "yl = 1×2\n\n    10    30",
            "loc = 'right'",
            "yl2 = 1×2\n\n     2     4",
            "k = 1",
            "sz = 1×2\n\n     2     1",
            "rc = 1×3\n\n    0.8500    0.3250    0.0980",
            "rl = 'R'",
            "rlim = 1×2\n\n     0    50",
            "ll = 1×2\n\n     2     4",
            "lc = 'left'",
            "lc2 = 'right'",
            "c1 = 1×3\n\n     0     0     0",
            "lab = 6×3 char array\n\n    'Jan'\n    'Feb'\n    'Mar'\n    'Apr'\n    'May'\n    'Jun'",
            "tk = 1×6\n\n      737791      737822      737851      737882      737912      737943",
            "lab2 = 11×4 char array\n\n    '2010'\n    '2011'\n    '2012'\n    '2013'\n    '2014'\n    '2015'\n    '2016'\n    '2017'\n    '2018'\n    '2019'\n    '2020'",
            "lab3 = 5×5 char array\n\n    '00:00'\n    '03:00'\n    '06:00'\n    '09:00'\n    '12:00'",
            "lab4 = 10×2 char array\n\n    '01'\n    '02'\n    '03'\n    '04'\n    '05'\n    '06'\n    '07'\n    '08'\n    '09'\n    '10'",
            "lab5 = 10×8 char array\n\n    '01/01/20'\n    '01/02/20'\n    '01/03/20'\n    '01/04/20'\n    '01/05/20'\n    '01/06/20'\n    '01/07/20'\n    '01/08/20'\n    '01/09/20'\n    '01/10/20'",
        ]
    );
}

#[test]
fn histcounts_and_displays_match_matlab() {
    assert_eq!(
        shown(concat!(
            "[c, e] = histcounts([1 2 3 4])\n[c2, e2] = histcounts([0.1 0.25 0.7 0.9 1.3])\n",
            "s = struct('a', zeros(1,0), 'b', [\"x\" \"yy\"], 'd', zeros(0,3), 'e', strings(1,0), 'g', [\"x\";\"y\"])\n",
            "x = [\"apple\";\"pear\";\"fig\"]\nk = categorical({'x','y','x'}); disp(k')\n",
        )),
        [
            "c = 1×4\n\n     1     1     1     1",
            "e = 1×5\n\n    0.5000    1.5000    2.5000    3.5000    4.5000",
            "c2 = 1×2\n\n     4     1",
            "e2 = 1×3\n\n     0     1     2",
            "s = struct with fields:\n\n    a: [1×0 double]\n    b: [\"x\"    \"yy\"]\n    d: [0×3 double]\n    e: [1×0 string]\n    g: [2×1 string]",
            "x = 3×1 string array\n\n    \"apple\"\n    \"pear\"\n    \"fig\"",
            "     x \n     y \n     x \n",
        ]
    );
}
