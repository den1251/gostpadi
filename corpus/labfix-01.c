/* ЛБ1, задача 1: линейный алгоритм.
 * Площадь и периметр треугольника по трём сторонам (формула Герона). */
#include <stdio.h>
#include <math.h>

int main(void) {
    double a = 0.0;
    double b = 0.0;
    double c = 0.0;
    double p = 0.0;
    double s = 0.0;

    printf("стороны треугольника a b c: ");
    scanf("%lf %lf %lf", &a, &b, &c);
    p = (a + b + c) / 2.0;
    s = sqrt(p * (p - a) * (p - b) * (p - c));
    printf("периметр = %.2lf\n", a + b + c);
    printf("полупериметр = %.2lf\n", p);
    printf("площадь = %.2lf\n", s);
    return 0;
}
